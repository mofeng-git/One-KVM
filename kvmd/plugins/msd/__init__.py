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


import os
import contextlib
import time

from typing import AsyncGenerator

import aiofiles
import aiofiles.os
import aiofiles.base

from ...logging import get_logger

from ...errors import OperationError
from ...errors import IsBusyError

from ... import aiotools

from .. import BasePlugin
from .. import get_plugin_class


# =====
class MsdError(Exception):
    pass


class MsdOperationError(OperationError, MsdError):
    pass


class MsdIsBusyError(IsBusyError, MsdError):
    def __init__(self) -> None:
        super().__init__("Performing another MSD operation, please try again later")


class MsdOfflineError(MsdOperationError):
    def __init__(self) -> None:
        super().__init__("MSD is not found")


class MsdConnectedError(MsdOperationError):
    def __init__(self) -> None:
        super().__init__("MSD is connected to Server, but shouldn't for this operation")


class MsdDisconnectedError(MsdOperationError):
    def __init__(self) -> None:
        super().__init__("MSD is disconnected from Server, but should be for this operation")


class MsdImageNotSelected(MsdOperationError):
    def __init__(self) -> None:
        super().__init__("The image is not selected")


class MsdUnknownImageError(MsdOperationError):
    def __init__(self) -> None:
        super().__init__("The image is not found in the storage")


class MsdImageExistsError(MsdOperationError):
    def __init__(self) -> None:
        super().__init__("This image is already exists")


# =====
class BaseMsdReader:
    def get_state(self) -> dict:
        raise NotImplementedError()

    def get_total_size(self) -> int:
        raise NotImplementedError()

    def get_chunk_size(self) -> int:
        raise NotImplementedError()

    async def read_chunked(self) -> AsyncGenerator[bytes, None]:
        if self is not None:  # XXX: Vulture and pylint hack
            raise NotImplementedError()
        yield


class BaseMsdWriter:
    def get_state(self) -> dict:
        raise NotImplementedError()

    def get_chunk_size(self) -> int:
        raise NotImplementedError()

    async def write_chunk(self, chunk: bytes) -> int:
        raise NotImplementedError()


class BaseMsd(BasePlugin):
    async def get_state(self) -> dict:
        raise NotImplementedError()

    async def poll_state(self) -> AsyncGenerator[dict, None]:
        if self is not None:  # XXX: Vulture and pylint hack
            raise NotImplementedError()
        yield

    async def reset(self) -> None:
        raise NotImplementedError()

    async def cleanup(self) -> None:
        pass

    # =====

    async def set_params(
        self,
        name: (str | None)=None,
        cdrom: (bool | None)=None,
        rw: (bool | None)=None,
    ) -> None:

        raise NotImplementedError()

    async def set_connected(self, connected: bool) -> None:
        raise NotImplementedError()

    @contextlib.asynccontextmanager
    async def read_image(self, name: str) -> AsyncGenerator[BaseMsdReader, None]:
        _ = name
        if self is not None:  # XXX: Vulture and pylint hack
            raise NotImplementedError()
        yield BaseMsdReader()

    @contextlib.asynccontextmanager
    async def write_image(self, name: str, size: int, remove_incomplete: (bool | None)) -> AsyncGenerator[BaseMsdWriter, None]:
        _ = name
        _ = size
        _ = remove_incomplete
        if self is not None:  # XXX: Vulture and pylint hack
            raise NotImplementedError()
        yield BaseMsdWriter()

    async def remove(self, name: str) -> None:
        raise NotImplementedError()


class MsdFileReader(BaseMsdReader):  # pylint: disable=too-many-instance-attributes
    def __init__(self, notifier: aiotools.AioNotifier, name: str, path: str, chunk_size: int) -> None:
        self.__notifier = notifier
        self.__name = name
        self.__path = path
        self.__chunk_size = chunk_size

        self.__file: (aiofiles.base.AiofilesContextManager | None) = None
        self.__file_size = 0
        self.__readed = 0
        self.__tick = 0.0

    def get_state(self) -> dict:
        return {
            "name": self.__name,
            "size": self.__file_size,
            "readed": self.__readed,
        }

    def get_total_size(self) -> int:
        assert self.__file is not None
        return self.__file_size

    def get_chunk_size(self) -> int:
        return self.__chunk_size

    async def read_chunked(self) -> AsyncGenerator[bytes, None]:
        assert self.__file is not None
        while True:
            chunk = await self.__file.read(self.__chunk_size)  # type: ignore
            if not chunk:
                break

            self.__readed += len(chunk)

            now = time.monotonic()
            if self.__tick + 1 < now or self.__readed == self.__file_size:
                self.__tick = now
                self.__notifier.notify()

            yield chunk

    async def open(self) -> "MsdFileReader":
        assert self.__file is None
        get_logger(1).info("Reading %r image from MSD ...", self.__name)
        self.__file_size = (await aiofiles.os.stat(self.__path)).st_size
        self.__file = await aiofiles.open(self.__path, mode="rb")  # type: ignore
        return self

    async def close(self) -> None:
        assert self.__file is not None
        logger = get_logger()
        logger.info("Closing image reader ...")
        try:
            await self.__file.close()  # type: ignore
        except Exception:
            logger.exception("Can't close image reader")


class MsdFileWriter(BaseMsdWriter):  # pylint: disable=too-many-instance-attributes
    def __init__(self, notifier: aiotools.AioNotifier, name: str, path: str, file_size: int, sync_size: int, chunk_size: int) -> None:
        self.__notifier = notifier
        self.__name = name
        self.__path = path
        self.__file_size = file_size
        self.__sync_size = sync_size
        self.__chunk_size = chunk_size

        self.__file: (aiofiles.base.AiofilesContextManager | None) = None
        self.__written = 0
        self.__unsynced = 0
        self.__tick = 0.0

    def get_state(self) -> dict:
        return {
            "name": self.__name,
            "size": self.__file_size,
            "written": self.__written,
        }

    def get_chunk_size(self) -> int:
        return self.__chunk_size

    async def write_chunk(self, chunk: bytes) -> int:
        assert self.__file is not None

        await self.__file.write(chunk)  # type: ignore
        self.__written += len(chunk)

        self.__unsynced += len(chunk)
        if self.__unsynced >= self.__sync_size:
            await self.__sync()
            self.__unsynced = 0

        now = time.monotonic()
        if self.__tick + 1 < now:
            self.__tick = now
            self.__notifier.notify()

        return self.__written

    def is_complete(self) -> bool:
        return (self.__written >= self.__file_size)

    async def open(self) -> "MsdFileWriter":
        assert self.__file is None
        get_logger(1).info("Writing %r image (%d bytes) to MSD ...", self.__name, self.__file_size)
        await aiofiles.os.makedirs(os.path.dirname(self.__path), exist_ok=True)
        self.__file = await aiofiles.open(self.__path, mode="w+b", buffering=0)  # type: ignore
        return self

    async def close(self) -> None:
        assert self.__file is not None
        logger = get_logger()
        logger.info("Closing image writer ...")
        try:
            if self.__written == self.__file_size:
                (log, result) = (logger.info, "OK")
            elif self.__written < self.__file_size:
                (log, result) = (logger.error, "INCOMPLETE")
            else:  # written > size
                (log, result) = (logger.warning, "OVERFLOW")
            log("Written %d of %d bytes to MSD image %r: %s", self.__written, self.__file_size, self.__name, result)
            try:
                await self.__sync()
            finally:
                await self.__file.close()  # type: ignore
        except Exception:
            logger.exception("Can't close image writer")

    async def __sync(self) -> None:
        assert self.__file is not None
        await self.__file.flush()  # type: ignore
        await aiotools.run_async(os.fsync, self.__file.fileno())  # type: ignore


# =====
def get_msd_class(name: str) -> type[BaseMsd]:
    return get_plugin_class("msd", name)  # type: ignore
