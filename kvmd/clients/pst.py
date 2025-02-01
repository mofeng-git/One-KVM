# ========================================================================== #
#                                                                            #
#    KVMD - The main PiKVM daemon.                                           #
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


import os
import contextlib

from typing import AsyncGenerator

import aiohttp

from .. import htclient
from .. import htserver


# =====
class PstError(Exception):
    pass


# =====
class PstClient:
    def __init__(
        self,
        subdir: str,
        unix_path: str,
        timeout: float,
        user_agent: str,
    ) -> None:

        self.__subdir = subdir
        self.__unix_path = unix_path
        self.__timeout = timeout
        self.__user_agent = user_agent

    async def get_path(self) -> str:
        async with self.__make_http_session() as session:
            async with session.get("http://localhost:0/state") as resp:
                htclient.raise_not_200(resp)
                path = (await resp.json())["result"]["data"]["path"]
                return os.path.join(path, self.__subdir)

    @contextlib.asynccontextmanager
    async def writable(self) -> AsyncGenerator[str, None]:
        async with self.__inner_writable() as path:
            path = os.path.join(path, self.__subdir)
            if not os.path.exists(path):
                os.mkdir(path)
            yield path

    @contextlib.asynccontextmanager
    async def __inner_writable(self) -> AsyncGenerator[str, None]:
        async with self.__make_http_session() as session:
            async with session.ws_connect("http://localhost:0/ws") as ws:
                path = ""
                async for msg in ws:
                    if msg.type != aiohttp.WSMsgType.TEXT:
                        raise PstError(f"Unexpected message type: {msg!r}")
                    (event_type, event) = htserver.parse_ws_event(msg.data)
                    if event_type == "storage":
                        if not event["data"]["write_allowed"]:
                            raise PstError("Write is not allowed")
                        path = event["data"]["path"]
                        break
                if not path:
                    raise PstError("WS loop broken without write_allowed=True flag")
                # TODO: Actually we should follow ws events, but for fast writing we can safely ignore them
                yield path

    def __make_http_session(self) -> aiohttp.ClientSession:
        return aiohttp.ClientSession(
            headers={"User-Agent": self.__user_agent},
            connector=aiohttp.UnixConnector(path=self.__unix_path),
            timeout=aiohttp.ClientTimeout(total=self.__timeout),
        )
