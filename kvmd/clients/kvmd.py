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
from typing import Union

import aiohttp

from .. import make_user_agent


# =====
class KvmdError(Exception):
    def __init__(self, err: Union[Exception, str]) -> None:
        if isinstance(err, Exception):
            super().__init__(f"{type(err).__name__}: {err}")
        else:
            super().__init__(err)


# =====
class _BaseClientPart:
    def __init__(
        self,
        host: str,
        port: int,
        unix_path: str,
        timeout: float,
    ) -> None:

        assert port or unix_path
        self.__host = host
        self.__port = port
        self.__unix_path = unix_path
        self.__timeout = timeout

    def _make_url(self, handle: str) -> str:
        assert not handle.startswith("/"), handle
        return f"http://{self.__host}:{self.__port}/{handle}"

    def _make_session(self, user: str, passwd: str) -> aiohttp.ClientSession:
        kwargs: Dict = {
            "headers": {
                "X-KVMD-User": user,
                "X-KVMD-Passwd": passwd,
                "User-Agent": make_user_agent("KVMD-VNC"),
            },
            "timeout": aiohttp.ClientTimeout(total=self.__timeout),
        }
        if self.__unix_path:
            kwargs["connector"] = aiohttp.UnixConnector(path=self.__unix_path)
        return aiohttp.ClientSession(**kwargs)


class _AuthClientPart(_BaseClientPart):
    async def check(self, user: str, passwd: str) -> bool:
        try:
            async with self._make_session(user, passwd) as session:
                async with session.get(self._make_url("auth/check")) as response:
                    response.raise_for_status()
                    if response.status == 200:
                        return True
                    raise KvmdError(f"Invalid OK response: {response.status} {await response.text()}")
        except aiohttp.ClientResponseError as err:
            if err.status in [401, 403]:
                return False
            raise KvmdError(err)
        except aiohttp.ClientError as err:
            raise KvmdError(err)


class _StreamerClientPart(_BaseClientPart):
    async def set_params(self, user: str, passwd: str, quality: int, desired_fps: int) -> None:
        try:
            async with self._make_session(user, passwd) as session:
                async with session.post(
                    url=self._make_url("streamer/set_params"),
                    params={"quality": quality, "desired_fps": desired_fps},
                ) as response:
                    response.raise_for_status()
        except aiohttp.ClientError as err:
            raise KvmdError(err)


# =====
class KvmdClient(_BaseClientPart):
    def __init__(
        self,
        host: str,
        port: int,
        unix_path: str,
        timeout: float,
    ) -> None:

        kwargs: Dict = {
            "host": host,
            "port": port,
            "unix_path": unix_path,
            "timeout": timeout,
        }

        super().__init__(**kwargs)

        self.auth = _AuthClientPart(**kwargs)
        self.streamer = _StreamerClientPart(**kwargs)

    @contextlib.asynccontextmanager
    async def ws(self, user: str, passwd: str) -> AsyncGenerator[aiohttp.ClientWebSocketResponse, None]:
        try:
            async with self._make_session(user, passwd) as session:
                async with session.ws_connect(self._make_url("ws")) as ws:
                    yield ws
        except aiohttp.ClientError as err:
            raise KvmdError(err)
