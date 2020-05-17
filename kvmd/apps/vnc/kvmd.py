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

from ... import __version__


# =====
class KvmdError(Exception):
    def __init__(self, err: Exception):
        super().__init__(f"{type(err).__name__} {err}")


# =====
class KvmdClient:
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

    # =====

    async def authorize(self, user: str, passwd: str) -> bool:
        try:
            async with self.__make_session(user, passwd) as session:
                async with session.get(
                    url=f"http://{self.__host}:{self.__port}/auth/check",
                    timeout=self.__timeout,
                ) as response:
                    response.raise_for_status()
                    if response.status == 200:
                        return True
                    raise RuntimeError(f"Invalid OK response: {response.status} {await response.text()}")
        except aiohttp.ClientResponseError as err:
            if err.status in [401, 403]:
                return False
            raise KvmdError(err)
        except aiohttp.ClientError as err:
            raise KvmdError(err)

    @contextlib.asynccontextmanager
    async def ws(self, user: str, passwd: str) -> AsyncGenerator[aiohttp.ClientWebSocketResponse, None]:
        try:
            async with self.__make_session(user, passwd) as session:
                async with session.ws_connect(
                    url=f"http://{self.__host}:{self.__port}/ws",
                    timeout=self.__timeout,
                ) as ws:
                    yield ws
        except aiohttp.ClientError as err:
            raise KvmdError(err)

    async def set_streamer_params(self, user: str, passwd: str, quality: int, desired_fps: int) -> None:
        try:
            async with self.__make_session(user, passwd) as session:
                async with session.post(
                    url=f"http://{self.__host}:{self.__port}/streamer/set_params",
                    timeout=self.__timeout,
                    params={
                        "quality": quality,
                        "desired_fps": desired_fps,
                    },
                ) as response:
                    response.raise_for_status()
        except aiohttp.ClientError as err:
            raise KvmdError(err)

    # =====

    def __make_session(self, user: str, passwd: str) -> aiohttp.ClientSession:
        kwargs: Dict = {
            "headers": {
                "X-KVMD-User": user,
                "X-KVMD-Passwd": passwd,
                "User-Agent": f"KVMD-VNC/{__version__}",
            },
        }
        if self.__unix_path:
            kwargs["connector"] = aiohttp.UnixConnector(path=self.__unix_path)
        return aiohttp.ClientSession(**kwargs)
