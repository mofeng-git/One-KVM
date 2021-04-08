# ========================================================================== #
#                                                                            #
#    KVMD - The main Pi-KVM daemon.                                          #
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

from typing import Dict
from typing import Optional

from pyghmi.ipmi.command import Command as IpmiCommand

from ...logging import get_logger

from ... import aiotools

from ...yamlconf import Option

from ...validators.basic import valid_float_f01
from ...validators.net import valid_ip_or_host
from ...validators.net import valid_port

from . import GpioDriverOfflineError
from . import BaseUserGpioDriver


# =====
_OUTPUTS = {
    1: "on",
    2: "off",
    3: "shutdown",
    4: "reset",
    5: "boot",
}


# =====
class Plugin(BaseUserGpioDriver):
    def __init__(  # pylint: disable=super-init-not-called
        self,
        instance_name: str,
        notifier: aiotools.AioNotifier,

        host: str,
        port: int,
        user: str,
        passwd: str,
        state_poll: float,
    ) -> None:

        super().__init__(instance_name, notifier)

        self.__host = host
        self.__port = port
        self.__user = user
        self.__passwd = passwd
        self.__state_poll = state_poll

        self.__online = False
        self.__power = False

    @classmethod
    def get_plugin_options(cls) -> Dict:
        return {
            "host":       Option("",  type=valid_ip_or_host),
            "port":       Option(623, type=valid_port),
            "user":       Option(""),
            "passwd":     Option(""),
            "state_poll": Option(1.0, type=valid_float_f01),
        }

    def register_input(self, pin: int, debounce: float) -> None:
        _ = debounce
        if pin != 0:
            raise RuntimeError(f"Unsupported mode 'input' for pin={pin} on {self}")

    def register_output(self, pin: int, initial: Optional[bool]) -> None:
        _ = initial
        if pin not in _OUTPUTS:
            raise RuntimeError(f"Unsupported mode 'output' for pin={pin} on {self}")

    def prepare(self) -> None:
        get_logger(0).info("Probing driver %s on %s:%d ...", self, self.__host, self.__port)

    async def run(self) -> None:
        prev = (False, False)
        while True:
            await aiotools.run_async(self.__update_power)
            new = (self.__online, self.__power)
            if new != prev:
                await self._notifier.notify()
                prev = new
            await asyncio.sleep(self.__state_poll)

    def cleanup(self) -> None:
        pass

    def read(self, pin: int) -> bool:
        if not self.__online:
            raise GpioDriverOfflineError(self)
        if pin == 0:
            return self.__power
        return False

    def write(self, pin: int, state: bool) -> None:
        if not self.__online:
            raise GpioDriverOfflineError(self)
        request = _OUTPUTS[pin]
        try:
            self.__make_command().set_power(request)
        except Exception:
            get_logger(0).exception("Can't send IPMI power-%s request to %s:%d", request, self.__host, self.__port)
            raise GpioDriverOfflineError(self)

    # =====

    def __update_power(self) -> None:
        try:
            self.__power = (self.__make_command().get_power()["powerstate"] == "on")
            self.__online = True
        except Exception:
            self.__online = self.__power = False
            get_logger(0).exception("Can't fetch IPMI power status from %s:%d", self.__host, self.__port)

    def __make_command(self) -> IpmiCommand:
        return IpmiCommand(
            bmc=self.__host,
            port=self.__port,
            userid=(self.__user or None),
            password=(self.__passwd or None),
        )

    def __str__(self) -> str:
        return f"IPMI({self._instance_name})"

    __repr__ = __str__
