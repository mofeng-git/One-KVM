# ========================================================================== #
#                                                                            #
#    KVMD - The main PiKVM daemon.                                           #
#                                                                            #
#    Copyright (C) 2018-2022  Maxim Devaev <mdevaev@gmail.com>               #
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

# At present this requires building a package from AUR:
# https://aur.archlinux.org/packages/python-pyserial-asyncio
# https://wiki.archlinux.org/title/Arch_User_Repository#Installing_and_upgrading_packages
import serial_asyncio

import functools

from typing import Tuple
from typing import Dict
from typing import Callable
from typing import Optional
from typing import Any

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

        mode: int,
        host: str,
        port: int,
        device_path: str,
        speed: int,
        timeout: float,
        switch_delay: float,
        state_poll: float,
    ) -> None:

        super().__init__(instance_name, notifier)

        self.__mode = mode
        self.__host = host
        self.__port = port
        self.__device_path = device_path
        self.__speed = speed
        self.__timeout = timeout
        self.__switch_delay = switch_delay
        self.__state_poll = state_poll

        self.__reader: Optional[asyncio.StreamReader] = None
        self.__writer: Optional[asyncio.StreamWriter] = None
        self.__active: int = -1
        self.__update_notifier = aiotools.AioNotifier()

    @classmethod
    def get_plugin_options(cls) -> Dict:
        return {
            "mode":         Option(1,    type=functools.partial(valid_number, min=1, max=2)),
            "host":         Option("",   type=valid_ip_or_host),
            "port":         Option(5000, type=valid_port),
            "device":       Option("",   type=valid_abs_path, unpack_as="device_path"),
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
                await self._notifier.notify()
                prev_active = self.__active
            await self.__update_notifier.wait(self.__state_poll)

    async def cleanup(self) -> None:
        await self.__close_device()

    async def read(self, pin: str) -> bool:
        return (self.__active == int(pin))

    async def write(self, pin: str, state: bool) -> None:
        # Switch input source command uses 1-based numbering (0x01->PC1...0x10->PC16)
        channel = int(pin)+1
        assert 1 <= channel <= 16
        if state:
            await self.__send_command("{:c}{:c}".format(1, channel).encode())
            await self.__update_notifier.notify()
            await asyncio.sleep(self.__switch_delay)  # Slowdown

    # =====

    async def __send_command(self, cmd: bytes) -> int:
        assert len(cmd) == 2
        (reader, writer) = await self.__ensure_device()
        try:
            writer.write(b"\xAA\xBB\x03%s\xEE" % (cmd))
            await asyncio.wait_for(writer.drain(), timeout=self.__timeout)
            return (await asyncio.wait_for(reader.readexactly(6), timeout=self.__timeout))[4]
        except Exception as err:
            get_logger(0).error("Can't send command to TESmart KVM [%s]:%d: %s",
                                self.__host, self.__port, tools.efmt(err))
            await self.__close_device()
            raise GpioDriverOfflineError(self)

    async def __ensure_device_tcpip(self) -> Tuple[asyncio.StreamReader, asyncio.StreamWriter]:
        try:
            (reader, writer) = await asyncio.wait_for(
                asyncio.open_connection(self.__host, self.__port),
                timeout=self.__timeout,
            )
            return (reader, writer)
        except Exception as err:
            get_logger(0).error("Can't connect to TESmart KVM [%s]:%d: %s",
                                self.__host, self.__port, tools.efmt(err))
            raise GpioDriverOfflineError(self)

    async def __ensure_device_serial(self) -> Tuple[asyncio.StreamReader, asyncio.StreamWriter]:
        try:
            (reader, writer) = await asyncio.wait_for(
                serial_asyncio.open_serial_connection(url=self.__device_path, baudrate=self.__speed),
                timeout=self.__timeout,
            )
            return (reader, writer)
        except Exception as err:
            get_logger(0).error("Can't connect to TESmart KVM [%s]:%d: %s",
                                self.__device_path, self.__speed, tools.efmt(err))
            raise GpioDriverOfflineError(self)

    async def __ensure_device(self) -> Tuple[asyncio.StreamReader, asyncio.StreamWriter]:
        if self.__reader is None or self.__writer is None:
            if self.__mode == 1:
                (self.__reader, self.__writer) = await self.__ensure_devicee_tcpip()
            elif self.__mode == 2:
                (self.__reader, self.__writer) = await self.__ensure_device_serial()
        return (self.__reader, self.__writer)

    async def __close_device(self) -> None:
        if self.__writer:
            await aiotools.close_writer(self.__writer)
        self.__reader = None
        self.__writer = None
        self.__active = -1

    # =====

    def __str__(self) -> str:
        return f"TESmart({self._instance_name})"

    __repr__ = __str__
