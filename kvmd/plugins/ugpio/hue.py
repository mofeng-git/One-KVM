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

from typing import Callable
from typing import Any

import aiohttp

from ...logging import get_logger

from ... import tools
from ... import aiotools
from ... import htclient

from ...yamlconf import Option

from ...validators.basic import valid_stripped_string_not_empty
from ...validators.basic import valid_bool
from ...validators.basic import valid_float_f01

from . import GpioDriverOfflineError
from . import BaseUserGpioDriver


# =====
class Plugin(BaseUserGpioDriver):  # pylint: disable=too-many-instance-attributes
    # https://developers.meethue.com/develop/hue-api/lights-api
    # https://www.burgestrand.se/hue-api/api/lights

    def __init__(
        self,
        instance_name: str,
        notifier: aiotools.AioNotifier,

        url: str,
        verify: bool,
        token: str,
        state_poll: float,
        timeout: float,
    ) -> None:

        super().__init__(instance_name, notifier)

        self.__url = url
        self.__verify = verify
        self.__token = token
        self.__state_poll = state_poll
        self.__timeout = timeout

        self.__initial: dict[str, (bool | None)] = {}

        self.__state: dict[str, (bool | None)] = {}
        self.__update_notifier = aiotools.AioNotifier()

        self.__http_session: (aiohttp.ClientSession | None) = None

    @classmethod
    def get_plugin_options(cls) -> dict:
        return {
            "url":        Option("",   type=valid_stripped_string_not_empty),
            "verify":     Option(True, type=valid_bool),
            "token":      Option("",   type=valid_stripped_string_not_empty),
            "state_poll": Option(5.0,  type=valid_float_f01),
            "timeout":    Option(5.0,  type=valid_float_f01),
        }

    @classmethod
    def get_pin_validator(cls) -> Callable[[Any], Any]:
        return valid_stripped_string_not_empty

    def register_input(self, pin: str, debounce: float) -> None:
        _ = debounce
        self.__state[pin] = None

    def register_output(self, pin: str, initial: (bool | None)) -> None:
        self.__initial[pin] = initial
        self.__state[pin] = None

    def prepare(self) -> None:
        async def inner_prepare() -> None:
            await asyncio.gather(*[
                self.write(pin, state)
                for (pin, state) in self.__initial.items()
                if state is not None
            ], return_exceptions=True)
        aiotools.run_sync(inner_prepare())

    async def run(self) -> None:
        prev_state: (dict | None) = None
        while True:
            session = self.__ensure_http_session()
            try:
                async with session.get(f"{self.__url}/api/{self.__token}/lights") as response:
                    results = await response.json()
                    for pin in self.__state:
                        if pin in results:
                            self.__state[pin] = bool(results[pin]["state"]["on"])
            except Exception as err:
                get_logger().error("Failed Hue bulk GET request: %s", tools.efmt(err))
                self.__state = dict.fromkeys(self.__state, None)
            if self.__state != prev_state:
                self._notifier.notify()
                prev_state = self.__state
            await self.__update_notifier.wait(self.__state_poll)

    async def cleanup(self) -> None:
        if self.__http_session:
            await self.__http_session.close()
            self.__http_session = None

    async def read(self, pin: str) -> bool:
        if self.__state[pin] is None:
            raise GpioDriverOfflineError(self)
        return self.__state[pin]  # type: ignore

    async def write(self, pin: str, state: bool) -> None:
        session = self.__ensure_http_session()
        try:
            async with session.put(
                url=f"{self.__url}/api/{self.__token}/lights/{pin}/state",
                json={"on": state},
            ) as response:
                htclient.raise_not_200(response)
        except Exception as err:
            get_logger().error("Failed Hue PUT request to pin %s: %s", pin, tools.efmt(err))
            raise GpioDriverOfflineError(self)
        self.__update_notifier.notify()

    def __ensure_http_session(self) -> aiohttp.ClientSession:
        if not self.__http_session:
            kwargs: dict = {
                "headers": {
                    "User-Agent": htclient.make_user_agent("KVMD"),
                },
                "timeout": aiohttp.ClientTimeout(total=self.__timeout),
            }
            if not self.__verify:
                kwargs["connector"] = aiohttp.TCPConnector(ssl=False)
            self.__http_session = aiohttp.ClientSession(**kwargs)
        return self.__http_session

    def __str__(self) -> str:
        return f"Hue({self._instance_name})"

    __repr__ = __str__
