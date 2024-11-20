# ========================================================================== #
#                                                                            #
#    KVMD - The main PiKVM daemon.                                           #
#                                                                            #
#    Copyright (C) 2018-2024  Maxim Devaev <mdevaev@gmail.com>               #
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


import types

from typing import Callable
from typing import Self

import aiohttp


# =====
class BaseHttpClientSession:
    def __init__(self, make_http_session: Callable[[], aiohttp.ClientSession]) -> None:
        self._make_http_session = make_http_session
        self.__http_session: (aiohttp.ClientSession | None) = None

    def _ensure_http_session(self) -> aiohttp.ClientSession:
        if not self.__http_session:
            self.__http_session = self._make_http_session()
        return self.__http_session

    async def close(self) -> None:
        if self.__http_session:
            await self.__http_session.close()
            self.__http_session = None

    async def __aenter__(self) -> Self:
        return self

    async def __aexit__(
        self,
        _exc_type: type[BaseException],
        _exc: BaseException,
        _tb: types.TracebackType,
    ) -> None:

        await self.close()


class BaseHttpClient:
    def __init__(
        self,
        unix_path: str,
        timeout: float,
        user_agent: str,
    ) -> None:

        self.__unix_path = unix_path
        self.__timeout = timeout
        self.__user_agent = user_agent

    def make_session(self) -> BaseHttpClientSession:
        raise NotImplementedError

    def _make_http_session(self, headers: (dict[str, str] | None)=None) -> aiohttp.ClientSession:
        return aiohttp.ClientSession(
            base_url="http://localhost:0",
            headers={
                "User-Agent": self.__user_agent,
                **(headers or {}),
            },
            connector=aiohttp.UnixConnector(path=self.__unix_path),
            timeout=aiohttp.ClientTimeout(total=self.__timeout),
        )
