# ========================================================================== #
#                                                                            #
#    KVMD - The main PiKVM daemon.                                           #
#                                                                            #
#    Copyright (C) 2018-2021  Maxim Devaev <mdevaev@gmail.com>               #
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
        timeout: float,
        switch_delay: float,
        state_poll: float,
    ) -> None:

        super().__init__(instance_name, notifier)

        self.__host = host
        self.__port = port
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
            "host":         Option("",   type=valid_ip_or_host),
            "port":         Option(5000, type=valid_port),
            "timeout":      Option(5.0,  type=valid_float_f01),
            "switch_delay": Option(1.0,  type=valid_float_f0),
            "state_poll":   Option(10.0, type=valid_float_f01),
        }

    @classmethod
    def get_pin_validator(cls) -> Callable[[Any], Any]:
        return functools.partial(valid_number, min=0, max=15, name="Tesmart channel")

    async def run(self) -> None:
        prev_active = -2
        while True:
            await self.__update_notifier.wait(self.__state_poll)
            try:
                self.__active = await self.__send_command(b"\x10\x00")
            except Exception:
                pass
            if self.__active != prev_active:
                await self._notifier.notify()
                prev_active = self.__active

    async def cleanup(self) -> None:
        await self.__close_device()

    async def read(self, pin: str) -> bool:
        return (self.__active == int(pin))

    async def write(self, pin: str, state: bool) -> None:
        channel = int(pin) + 1
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
            get_logger(0).error("Can't send command to Tesmart KVM [%s]:%d: %s",
                                self.__host, self.__port, tools.efmt(err))
            await self.__close_device()
            raise GpioDriverOfflineError(self)

    async def __ensure_device(self) -> Tuple[asyncio.StreamReader, asyncio.StreamWriter]:
        if self.__reader is None or self.__writer is None:
            try:
                (reader, writer) = await asyncio.wait_for(
                    asyncio.open_connection(self.__host, self.__port),
                    timeout=self.__timeout,
                )
            except Exception as err:
                get_logger(0).error("Can't connect to Tesmart KVM [%s]:%d: %s",
                                    self.__host, self.__port, tools.efmt(err))
                raise GpioDriverOfflineError(self)
            else:
                self.__reader = reader
                self.__writer = writer
        return (self.__reader, self.__writer)

    async def __close_device(self) -> None:
        if self.__writer:
            await aiotools.close_writer(self.__writer)
        self.__reader = None
        self.__writer = None
        self.__active = -1

    # =====

    def __str__(self) -> str:
        return f"Tesmart({self._instance_name})"

    __repr__ = __str__
