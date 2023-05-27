# ========================================================================== #
#                                                                            #
#    KVMD - The main PiKVM daemon.                                           #
#                                                                            #
#    Copyright (C) 2018-2023  Maxim Devaev <mdevaev@gmail.com>               #
#                                                                            #
#    Modified by SppokHCK September 2021 <Find me on Discord spook#8911>     #
#                                                                            #
#    This program is free software: you can redistribute it and/or modify    #
#    it under the terms of the GNU General Public License as published by    #
#    the Free Software Foundation, either version 3 of the License, or       #
#    (at your option) any later version.                                     #
#                                                                            #
#    This program is distributed in the hope that it will be useful,         #
#    but WITHOUT ANY WARRANTY; without even the implied warranty of          #
#    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the           #
#    GNU General Public License for more details.                            #
#                                                                            #
#    You should have received a copy of the GNU General Public License       #
#    along with this program.  If not, see <https://www.gnu.org/licenses/>.  #
#                                                                            #
# ========================================================================== #


import re
import multiprocessing
import functools
import errno
import time

from typing import Callable
from typing import Any

import serial

from ...logging import get_logger

from ... import aiotools
from ... import aiomulti
from ... import aioproc

from ...yamlconf import Option

from ...validators.basic import valid_number
from ...validators.basic import valid_float_f01
from ...validators.os import valid_abs_path
from ...validators.hw import valid_tty_speed

from . import GpioDriverOfflineError
from . import BaseUserGpioDriver


# =====
class Plugin(BaseUserGpioDriver):  # pylint: disable=too-many-instance-attributes
    def __init__(
        self,
        instance_name: str,
        notifier: aiotools.AioNotifier,

        device_path: str,
        speed: int,
        read_timeout: float,
        protocol: int,
    ) -> None:

        super().__init__(instance_name, notifier)

        self.__device_path = device_path
        self.__speed = speed
        self.__read_timeout = read_timeout
        self.__protocol = protocol

        self.__ctl_queue: "multiprocessing.Queue[int]" = multiprocessing.Queue()
        self.__channel_queue: "multiprocessing.Queue[int | None]" = multiprocessing.Queue()
        self.__channel: (int | None) = -1

        self.__proc: (multiprocessing.Process | None) = None
        self.__stop_event = multiprocessing.Event()

    @classmethod
    def get_plugin_options(cls) -> dict:
        return {
            "device":       Option("",    type=valid_abs_path, unpack_as="device_path"),
            "speed":        Option(19200, type=valid_tty_speed),
            "read_timeout": Option(2.0,   type=valid_float_f01),
            "protocol":     Option(1,     type=functools.partial(valid_number, min=1, max=2)),
        }

    @classmethod
    def get_pin_validator(cls) -> Callable[[Any], Any]:
        return functools.partial(valid_number, min=0, max=15, name="PWAY channel")

    def prepare(self) -> None:
        assert self.__proc is None
        self.__proc = multiprocessing.Process(target=self.__serial_worker, daemon=True)
        self.__proc.start()

    async def run(self) -> None:
        while True:
            (got, channel) = await aiomulti.queue_get_last(self.__channel_queue, 1)
            if got and self.__channel != channel:
                self.__channel = channel
                self._notifier.notify()

    async def cleanup(self) -> None:
        if self.__proc is not None:
            if self.__proc.is_alive():
                get_logger(0).info("Stopping %s daemon ...", self)
                self.__stop_event.set()
            if self.__proc.is_alive() or self.__proc.exitcode is not None:
                self.__proc.join()

    async def read(self, pin: str) -> bool:
        if not self.__is_online():
            raise GpioDriverOfflineError(self)
        return (self.__channel == int(pin))

    async def write(self, pin: str, state: bool) -> None:
        if not self.__is_online():
            raise GpioDriverOfflineError(self)
        if state:
            self.__ctl_queue.put_nowait(int(pin))

    # =====

    def __is_online(self) -> bool:
        return (
            self.__proc is not None
            and self.__proc.is_alive()
            and self.__channel is not None
        )

    def __serial_worker(self) -> None:
        logger = aioproc.settle(str(self), f"gpio-pway-{self._instance_name}")
        while not self.__stop_event.is_set():
            try:
                with self.__get_serial() as tty:
                    data = b""
                    self.__channel_queue.put_nowait(-1)

                    # Switch and then recieve the state.
                    # FIXME: Get actual state without modifying the current.
                    # I'm lazy and like the idea of the KVM resetting to port 1 on reboot of the PiKVM.
                    self.__reset(tty)

                    while not self.__stop_event.is_set():
                        (channel, data) = self.__recv_channel(tty, data)
                        if channel is not None:
                            self.__channel_queue.put_nowait(channel)

                        (got, channel) = aiomulti.queue_get_last_sync(self.__ctl_queue, 0.1)  # type: ignore
                        if got:
                            assert channel is not None
                            self.__send_channel(tty, channel)

            except Exception as err:
                self.__channel_queue.put_nowait(None)
                if isinstance(err, serial.SerialException) and err.errno == errno.ENOENT:  # pylint: disable=no-member
                    logger.error("Missing %s serial device: %s", self, self.__device_path)
                else:
                    logger.exception("Unexpected %s error", self)
                time.sleep(1)

    def __get_serial(self) -> serial.Serial:
        return serial.Serial(self.__device_path, self.__speed, timeout=self.__read_timeout)

    def __recv_channel(self, tty: serial.Serial, data: bytes) -> tuple[(int | None), bytes]:
        channel: (int | None) = None
        if tty.in_waiting:
            data += tty.read_all()
            # When you switch ports you see something like "VGA_SWITCH_CONTROL=[0-15]" for ports 1-16
            found = re.findall(b"VGA_SWITCH_CONTROL=[0-9]*", data)
            if found:
                channel = int(found[0].decode().split("=")[1])
            data = data[-8:]
        return (channel, data)

    def __send_channel(self, tty: serial.Serial, channel: int) -> None:
        # Set a channel by sending PS [1-16]
        # Note that the recv is 0-based index, while send is 1-based
        # We add 1 to the "channel" to normalize for the 1-based index on send
        tty.write(b"PS %d\r" % (channel + 1))
        tty.flush()

    def __reset(self, tty: serial.Serial) -> None:
        # Reset by sending PS without port number
        tty.write(b"PS\r")
        tty.flush()

    def __str__(self) -> str:
        return f"PWAY({self._instance_name})"

    __repr__ = __str__
