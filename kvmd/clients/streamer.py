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


from typing import Tuple
from typing import Dict
from typing import AsyncGenerator

import aiohttp

from .. import aiotools


# =====
class StreamerError(Exception):
    def __init__(self, err: Exception):
        super().__init__(f"{type(err).__name__}: {err}")


# =====
class StreamerClient:
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

    async def read(self) -> AsyncGenerator[Tuple[bool, int, int, bytes], None]:
        try:
            async with self.__make_session() as session:
                async with session.get(
                    url=f"http://{self.__host}:{self.__port}/stream",
                    params={"extra_headers": "1"},
                    headers={"User-Agent": self.__user_agent},
                ) as response:
                    aiotools.raise_not_200(response)
                    reader = aiohttp.MultipartReader.from_response(response)
                    while True:
                        frame = await reader.next()  # pylint: disable=not-callable
                        if not isinstance(frame, aiohttp.BodyPartReader):
                            raise RuntimeError("Expected body part")
                        yield (
                            (frame.headers["X-UStreamer-Online"] == "true"),
                            int(frame.headers["X-UStreamer-Width"]),
                            int(frame.headers["X-UStreamer-Height"]),
                            bytes(await frame.read()),
                        )
        except Exception as err:  # Тут бывают и ассерты, и KeyError, и прочая херня из-за корявых исключений в MultipartReader
            raise StreamerError(err)

    def __make_session(self) -> aiohttp.ClientSession:
        kwargs: Dict = {
            "timeout": aiohttp.ClientTimeout(
                connect=self.__timeout,
                sock_read=self.__timeout,
            ),
        }
        if self.__unix_path:
            kwargs["connector"] = aiohttp.UnixConnector(path=self.__unix_path)
        return aiohttp.ClientSession(**kwargs)
