# ========================================================================== #
#                                                                            #
#    KVMD - The main PiKVM daemon.                                           #
#                                                                            #
#    Copyright (C) 2018-2023  Maxim Devaev <mdevaev@gmail.com>               #
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


import asyncio
import functools

from typing import Callable
from typing import Any

import serial_asyncio

from ...logging import get_logger

from ... import tools
from ... import aiotools

from ...yamlconf import Option

from ...validators.basic import valid_number
from ...validators.basic import valid_float_f0
from ...validators.basic import valid_float_f01
from ...validators.net import valid_ip_or_host
from ...validators.net import valid_port
from ...validators.os import valid_abs_path
from ...validators.hw import valid_tty_speed

from . import BaseUserGpioDriver
from . import GpioDriverOfflineError


# =====
class Plugin(BaseUserGpioDriver):  # pylint: disable=too-many-instance-attributes
    def __init__(
        self,
        instance_name: str,
        notifier: aiotools.AioNotifier,

        host: str,
        port: int,

        device_path: str,
        speed: int,

        timeout: float,
        switch_delay: float,
        state_poll: float,
    ) -> None:

        super().__init__(instance_name, notifier)

        self.__host = host
        self.__port = port

        self.__device_path = device_path
        self.__speed = speed

        self.__timeout = timeout
        self.__switch_delay = switch_delay
        self.__state_poll = state_poll

        self.__reader: (asyncio.StreamReader | None) = None
        self.__writer: (asyncio.StreamWriter | None) = None
        self.__active: int = -1
        self.__update_notifier = aiotools.AioNotifier()

    @classmethod
    def get_plugin_options(cls) -> dict:
        return {
            "host":         Option("",   type=valid_ip_or_host, if_empty=""),
            "port":         Option(5000, type=valid_port),

            "device":       Option("",   type=valid_abs_path, only_if="!host", unpack_as="device_path"),
            "speed":        Option(9600, type=valid_tty_speed),

            "timeout":      Option(5.0,  type=valid_float_f01),
            "switch_delay": Option(1.0,  type=valid_float_f0),
            "state_poll":   Option(10.0, type=valid_float_f01),
        }

    @classmethod
    def get_pin_validator(cls) -> Callable[[Any], Any]:
        return functools.partial(valid_number, min=0, max=15, name="TESmart channel")

    async def run(self) -> None:
        prev_active = -2
        while True:
            try:
                # Current active port command uses 0-based numbering (0x00->PC1...0x0F->PC16)
                self.__active = int(await self.__send_command(b"\x10\x00"))
            except Exception:
                pass
            if self.__active != prev_active:
                self._notifier.notify()
                prev_active = self.__active
            await self.__update_notifier.wait(self.__state_poll)

    async def cleanup(self) -> None:
        await self.__close_device()
        self.__active = -1

    async def read(self, pin: str) -> bool:
        return (self.__active == int(pin))

    async def write(self, pin: str, state: bool) -> None:
        # Switch input source command uses 1-based numbering (0x01->PC1...0x10->PC16)
        channel = int(pin) + 1
        assert 1 <= channel <= 16
        if state:
            await self.__send_command("{:c}{:c}".format(1, channel).encode())
            await asyncio.sleep(self.__switch_delay)  # Slowdown
            self.__update_notifier.notify()

    # =====

    async def __send_command(self, cmd: bytes) -> int:
        assert len(cmd) == 2
        await self.__ensure_device()
        assert self.__reader is not None
        assert self.__writer is not None
        try:
            self.__writer.write(b"\xAA\xBB\x03%s\xEE" % (cmd))
            await asyncio.wait_for(
                asyncio.ensure_future(self.__writer.drain()),
                timeout=self.__timeout,
            )
            return (await asyncio.wait_for(
                asyncio.ensure_future(self.__reader.readexactly(6)),
                timeout=self.__timeout,
            ))[4]
        except Exception as err:
            get_logger(0).error("Can't send command to TESmart KVM [%s]:%d: %s",
                                self.__host, self.__port, tools.efmt(err))
            await self.__close_device()
            self.__active = -1
            raise GpioDriverOfflineError(self)
        finally:
            await self.__close_device()

    async def __ensure_device(self) -> None:
        if self.__reader is None or self.__writer is None:
            if self.__host:
                await self.__ensure_device_net()
            else:
                await self.__ensure_device_serial()

    async def __ensure_device_net(self) -> None:
        try:
            (self.__reader, self.__writer) = await asyncio.wait_for(
                asyncio.ensure_future(asyncio.open_connection(self.__host, self.__port)),
                timeout=self.__timeout,
            )
        except Exception as err:
            get_logger(0).error("Can't connect to TESmart KVM [%s]:%d: %s",
                                self.__host, self.__port, tools.efmt(err))
            raise GpioDriverOfflineError(self)

    async def __ensure_device_serial(self) -> None:
        try:
            (self.__reader, self.__writer) = await asyncio.wait_for(
                serial_asyncio.open_serial_connection(url=self.__device_path, baudrate=self.__speed),
                timeout=self.__timeout,
            )
        except Exception as err:
            get_logger(0).error("Can't connect to TESmart KVM [%s]:%d: %s",
                                self.__device_path, self.__speed, tools.efmt(err))
            raise GpioDriverOfflineError(self)

    async def __close_device(self) -> None:
        if self.__writer:
            await aiotools.close_writer(self.__writer)
        self.__reader = None
        self.__writer = None

    # =====

    def __str__(self) -> str:
        return f"TESmart({self._instance_name})"

    __repr__ = __str__
