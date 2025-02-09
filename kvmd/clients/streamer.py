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


import io
import contextlib
import dataclasses
import functools
import types

from typing import Callable
from typing import Awaitable
from typing import Generator
from typing import AsyncGenerator

import aiohttp
import ustreamer

from PIL import Image as PilImage

from .. import tools
from .. import aiotools
from .. import htclient

from . import BaseHttpClient
from . import BaseHttpClientSession


# =====
class StreamerError(Exception):
    pass


class StreamerTempError(StreamerError):
    pass


class StreamerPermError(StreamerError):
    pass


# =====
class StreamerFormats:
    JPEG = 1195724874    # V4L2_PIX_FMT_JPEG
    H264 = 875967048     # V4L2_PIX_FMT_H264
    _MJPEG = 1196444237  # V4L2_PIX_FMT_MJPEG

    @classmethod
    def is_diff(cls, fmt: int) -> bool:
        return (fmt == cls.H264)


class BaseStreamerClient:
    def get_format(self) -> int:
        raise NotImplementedError()

    @contextlib.asynccontextmanager
    async def reading(self) -> AsyncGenerator[Callable[[bool], Awaitable[dict]], None]:
        if self is not None:  # XXX: Vulture and pylint hack
            raise NotImplementedError()
        yield


# =====
@dataclasses.dataclass(frozen=True)
class StreamerSnapshot:
    online: bool
    width: int
    height: int
    headers: tuple[tuple[str, str], ...]
    data: bytes

    async def make_preview(self, max_width: int, max_height: int, quality: int) -> bytes:
        assert max_width >= 0
        assert max_height >= 0
        assert quality > 0

        if max_width == 0 and max_height == 0:
            max_width = self.width // 5
            max_height = self.height // 5
        else:
            max_width = min((max_width or self.width), self.width)
            max_height = min((max_height or self.height), self.height)

        if (max_width, max_height) == (self.width, self.height):
            return self.data
        return (await aiotools.run_async(self.__inner_make_preview, max_width, max_height, quality))

    @functools.lru_cache(maxsize=1)
    def __inner_make_preview(self, max_width: int, max_height: int, quality: int) -> bytes:
        with io.BytesIO(self.data) as snapshot_bio:
            with io.BytesIO() as preview_bio:
                with PilImage.open(snapshot_bio) as image:
                    image.thumbnail((max_width, max_height), PilImage.Resampling.LANCZOS)
                    image.save(preview_bio, format="jpeg", quality=quality)
                    return preview_bio.getvalue()


class HttpStreamerClientSession(BaseHttpClientSession):
    async def get_state(self) -> dict:
        session = self._ensure_http_session()
        async with session.get("/state") as resp:
            htclient.raise_not_200(resp)
            return (await resp.json())["result"]

    async def take_snapshot(self, timeout: float) -> StreamerSnapshot:
        session = self._ensure_http_session()
        async with session.get(
            url="/snapshot",
            timeout=aiohttp.ClientTimeout(total=timeout),
        ) as resp:

            htclient.raise_not_200(resp)
            return StreamerSnapshot(
                online=(resp.headers["X-UStreamer-Online"] == "true"),
                width=int(resp.headers["X-UStreamer-Width"]),
                height=int(resp.headers["X-UStreamer-Height"]),
                headers=tuple(
                    (key, value)
                    for (key, value) in tools.sorted_kvs(dict(resp.headers))
                    if key.lower().startswith("x-ustreamer-") or key.lower() in [
                        "x-timestamp",
                        "access-control-allow-origin",
                        "cache-control",
                        "pragma",
                        "expires",
                    ]
                ),
                data=bytes(await resp.read()),
            )


@contextlib.contextmanager
def _http_reading_handle_errors() -> Generator[None, None, None]:
    try:
        yield
    except Exception as ex:  # Тут бывают и ассерты, и KeyError, и прочая херня
        if isinstance(ex, StreamerTempError):
            raise
        raise StreamerTempError(tools.efmt(ex))


class HttpStreamerClient(BaseHttpClient, BaseStreamerClient):
    def __init__(
        self,
        name: str,
        unix_path: str,
        timeout: float,
        user_agent: str,
    ) -> None:

        super().__init__(unix_path, timeout, user_agent)
        self.__name = name

    def make_session(self) -> HttpStreamerClientSession:
        return HttpStreamerClientSession(self._make_http_session)

    def get_format(self) -> int:
        return StreamerFormats.JPEG

    @contextlib.asynccontextmanager
    async def reading(self) -> AsyncGenerator[Callable[[bool], Awaitable[dict]], None]:
        with _http_reading_handle_errors():
            async with self._make_http_session() as session:
                async with session.get(
                    url="/stream",
                    params={"extra_headers": "1"},
                    timeout=aiohttp.ClientTimeout(
                        connect=session.timeout.total,
                        sock_read=session.timeout.total,
                    ),
                ) as resp:

                    htclient.raise_not_200(resp)
                    reader = aiohttp.MultipartReader.from_response(resp)
                    self.__patch_stream_reader(reader.resp.content)

                    async def read_frame(key_required: bool) -> dict:
                        _ = key_required
                        with _http_reading_handle_errors():
                            frame = await reader.next()  # pylint: disable=not-callable
                            if not isinstance(frame, aiohttp.BodyPartReader):
                                raise StreamerTempError("Expected body part")

                            data = bytes(await frame.read())
                            if not data:
                                raise StreamerTempError("Reached EOF")

                            return {
                                "online": (frame.headers["X-UStreamer-Online"] == "true"),
                                "width": int(frame.headers["X-UStreamer-Width"]),
                                "height": int(frame.headers["X-UStreamer-Height"]),
                                "data": data,
                                "format": StreamerFormats.JPEG,
                            }

                    yield read_frame

    def __patch_stream_reader(self, reader: aiohttp.StreamReader) -> None:
        # https://github.com/pikvm/pikvm/issues/92
        # Infinite looping in BodyPartReader.read() because _at_eof flag.

        orig_read = reader.read

        async def read(self: aiohttp.StreamReader, n: int=-1) -> bytes:  # pylint: disable=invalid-name
            if self.is_eof():
                raise StreamerTempError("StreamReader.read(): Reached EOF")
            return (await orig_read(n))

        reader.read = types.MethodType(read, reader)  # type: ignore

    def __str__(self) -> str:
        return f"HttpStreamerClient({self.__name})"


# =====
@contextlib.contextmanager
def _memsink_reading_handle_errors() -> Generator[None, None, None]:
    try:
        yield
    except StreamerPermError:
        raise
    except FileNotFoundError as ex:
        raise StreamerTempError(tools.efmt(ex))
    except Exception as ex:
        raise StreamerPermError(tools.efmt(ex))


class MemsinkStreamerClient(BaseStreamerClient):
    def __init__(
        self,
        name: str,
        fmt: int,
        obj: str,
        lock_timeout: float,
        wait_timeout: float,
        drop_same_frames: float,
    ) -> None:

        self.__name = name
        self.__fmt = fmt
        self.__kwargs: dict = {
            "obj": obj,
            "lock_timeout": lock_timeout,
            "wait_timeout": wait_timeout,
            "drop_same_frames": drop_same_frames,
        }

    def get_format(self) -> int:
        return self.__fmt

    @contextlib.asynccontextmanager
    async def reading(self) -> AsyncGenerator[Callable[[bool], Awaitable[dict]], None]:
        with _memsink_reading_handle_errors():
            with ustreamer.Memsink(**self.__kwargs) as sink:
                async def read_frame(key_required: bool) -> dict:
                    key_required = (key_required and self.__fmt == StreamerFormats.H264)
                    with _memsink_reading_handle_errors():
                        while True:
                            frame = await aiotools.run_async(sink.wait_frame, key_required)
                            if frame is not None:
                                self.__check_format(frame["format"])
                                return frame
                yield read_frame

    def __check_format(self, fmt: int) -> None:
        if fmt == StreamerFormats._MJPEG:  # pylint: disable=protected-access
            fmt = StreamerFormats.JPEG
        if fmt != self.__fmt:
            raise StreamerPermError("Invalid sink format")

    def __str__(self) -> str:
        return f"MemsinkStreamerClient({self.__name})"
