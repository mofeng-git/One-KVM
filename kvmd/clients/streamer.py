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


import types

from typing import Tuple
from typing import Dict
from typing import AsyncGenerator

import aiohttp

from .. import htclient


# =====
class StreamerError(Exception):
    pass


# =====
def _patch_stream_reader(reader: aiohttp.StreamReader) -> None:
    # https://github.com/pikvm/pikvm/issues/92
    # Infinite looping in BodyPartReader.read() because _at_eof flag.

    orig_read = reader.read

    async def read(self: aiohttp.StreamReader, n: int=-1) -> bytes:  # pylint: disable=invalid-name
        if self.is_eof():
            raise StreamerError("StreamReader.read(): Reached EOF")
        return (await orig_read(n))

    reader.read = types.MethodType(read, reader)  # type: ignore


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

    async def read_stream(self) -> AsyncGenerator[Tuple[bool, int, int, bytes], None]:
        try:
            async with self.__make_http_session() as session:
                async with session.get(
                    url=self.__make_url("stream"),
                    params={"extra_headers": "1"},
                ) as response:
                    htclient.raise_not_200(response)
                    reader = aiohttp.MultipartReader.from_response(response)
                    _patch_stream_reader(reader.resp.content)

                    while True:
                        frame = await reader.next()  # pylint: disable=not-callable
                        if not isinstance(frame, aiohttp.BodyPartReader):
                            raise StreamerError("Expected body part")

                        data = bytes(await frame.read())
                        if not data:
                            break

                        yield (
                            (frame.headers["X-UStreamer-Online"] == "true"),
                            int(frame.headers["X-UStreamer-Width"]),
                            int(frame.headers["X-UStreamer-Height"]),
                            data,
                        )
        except Exception as err:  # Тут бывают и ассерты, и KeyError, и прочая херня
            if isinstance(err, StreamerError):
                raise
            raise StreamerError(f"{type(err).__name__}: {err}")
        raise StreamerError("Reached EOF")

    def __make_http_session(self) -> aiohttp.ClientSession:
        kwargs: Dict = {
            "headers": {"User-Agent": self.__user_agent},
            "timeout": aiohttp.ClientTimeout(
                connect=self.__timeout,
                sock_read=self.__timeout,
            ),
        }
        if self.__unix_path:
            kwargs["connector"] = aiohttp.UnixConnector(path=self.__unix_path)
        return aiohttp.ClientSession(**kwargs)

    def __make_url(self, handle: str) -> str:
        assert not handle.startswith("/"), handle
        return f"http://{self.__host}:{self.__port}/{handle}"
