# ========================================================================== #
#                                                                            #
#    KVMD - The main Pi-KVM daemon.                                          #
#                                                                            #
#    Copyright (C) 2020  Maxim Devaev <mdevaev@gmail.com>                    #
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


import contextlib

from typing import Dict
from typing import AsyncGenerator

import aiohttp

from .. import aiotools


# =====
class _BaseClientPart:
    def __init__(
        self,
        host: str,
        port: int,
        unix_path: str,
        timeout: float,
        user_agent: str,
    ) -> None:

        assert port or unix_path
        self.__host = host
        self.__port = port
        self.__unix_path = unix_path
        self.__timeout = timeout
        self.__user_agent = user_agent

    def _make_session(self, user: str, passwd: str) -> aiohttp.ClientSession:
        kwargs: Dict = {
            "headers": {
                "X-KVMD-User": user,
                "X-KVMD-Passwd": passwd,
                "User-Agent": self.__user_agent,
            },
            "timeout": aiohttp.ClientTimeout(total=self.__timeout),
        }
        if self.__unix_path:
            kwargs["connector"] = aiohttp.UnixConnector(path=self.__unix_path)
        return aiohttp.ClientSession(**kwargs)

    def _make_url(self, handle: str) -> str:
        assert not handle.startswith("/"), handle
        return f"http://{self.__host}:{self.__port}/{handle}"


class _AuthClientPart(_BaseClientPart):
    async def check(self, user: str, passwd: str) -> bool:
        try:
            async with self._make_session(user, passwd) as session:
                async with session.get(self._make_url("auth/check")) as response:
                    aiotools.raise_not_200(response)
                    return True
        except aiohttp.ClientResponseError as err:
            if err.status in [401, 403]:
                return False
            raise


class _StreamerClientPart(_BaseClientPart):
    async def set_params(self, user: str, passwd: str, quality: int, desired_fps: int) -> None:
        async with self._make_session(user, passwd) as session:
            async with session.post(
                url=self._make_url("streamer/set_params"),
                params={"quality": quality, "desired_fps": desired_fps},
            ) as response:
                aiotools.raise_not_200(response)


class _HidClientPart(_BaseClientPart):
    async def print(self, user: str, passwd: str, text: str, limit: int) -> None:
        async with self._make_session(user, passwd) as session:
            async with session.post(
                url=self._make_url("hid/print"),
                params={"limit": limit},
                data=text,
            ) as response:
                aiotools.raise_not_200(response)


class _AtxClientPart(_BaseClientPart):
    async def get_state(self, user: str, passwd: str) -> Dict:
        async with self._make_session(user, passwd) as session:
            async with session.get(self._make_url("atx")) as response:
                aiotools.raise_not_200(response)
                return (await response.json())["result"]

    async def switch_power(self, user: str, passwd: str, action: str) -> bool:
        try:
            async with self._make_session(user, passwd) as session:
                async with session.post(
                    url=self._make_url("atx/power"),
                    params={"action": action},
                ) as response:
                    aiotools.raise_not_200(response)
                    return True
        except aiohttp.ClientResponseError as err:
            if err.status == 409:
                return False
            raise


# =====
class KvmdClient(_BaseClientPart):
    def __init__(
        self,
        host: str,
        port: int,
        unix_path: str,
        timeout: float,
        user_agent: str,
    ) -> None:

        kwargs: Dict = {
            "host": host,
            "port": port,
            "unix_path": unix_path,
            "timeout": timeout,
            "user_agent": user_agent,
        }

        super().__init__(**kwargs)

        self.auth = _AuthClientPart(**kwargs)
        self.streamer = _StreamerClientPart(**kwargs)
        self.hid = _HidClientPart(**kwargs)
        self.atx = _AtxClientPart(**kwargs)

    @contextlib.asynccontextmanager
    async def ws(self, user: str, passwd: str) -> AsyncGenerator[aiohttp.ClientWebSocketResponse, None]:
        async with self._make_session(user, passwd) as session:
            async with session.ws_connect(self._make_url("ws")) as ws:
                yield ws
