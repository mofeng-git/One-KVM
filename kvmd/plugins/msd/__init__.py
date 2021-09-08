# ========================================================================== #
#                                                                            #
#    KVMD - The main PiKVM daemon.                                           #
#                                                                            #
#    Copyright (C) 2018-2021  Maxim Devaev <mdevaev@gmail.com>               #
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

from typing import Dict
from typing import Type
from typing import AsyncGenerator
from typing import Optional

import aiofiles
import aiofiles.base

from ...logging import get_logger

from ... import aiofs

from ...errors import OperationError
from ...errors import IsBusyError

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


class MsdMultiNotSupported(MsdOperationError):
    def __init__(self) -> None:
        super().__init__("This MSD does not support storing multiple images")


class MsdCdromNotSupported(MsdOperationError):
    def __init__(self) -> None:
        super().__init__("This MSD does not support CD-ROM emulation")


# =====
class BaseMsd(BasePlugin):
    async def get_state(self) -> Dict:
        raise NotImplementedError()

    async def poll_state(self) -> AsyncGenerator[Dict, None]:
        if self is not None:  # XXX: Vulture and pylint hack
            raise NotImplementedError()
        yield

    async def reset(self) -> None:
        raise NotImplementedError()

    async def cleanup(self) -> None:
        pass

    # =====

    async def set_params(self, name: Optional[str]=None, cdrom: Optional[bool]=None) -> None:
        raise NotImplementedError()

    async def set_connected(self, connected: bool) -> None:
        raise NotImplementedError()

    @contextlib.asynccontextmanager
    async def write_image(self, name: str, size: int) -> AsyncGenerator[int, None]:  # pylint: disable=unused-argument
        if self is not None:  # XXX: Vulture and pylint hack
            raise NotImplementedError()
        yield 1

    async def write_image_chunk(self, chunk: bytes) -> int:
        raise NotImplementedError()

    async def remove(self, name: str) -> None:
        raise NotImplementedError()


class MsdImageWriter:
    def __init__(self, path: str, size: int, sync: int) -> None:
        self.__name = os.path.basename(path)
        self.__path = path
        self.__size = size
        self.__sync = sync

        self.__file: Optional[aiofiles.base.AiofilesContextManager] = None
        self.__written = 0
        self.__unsynced = 0

    def get_file(self) -> aiofiles.base.AiofilesContextManager:
        assert self.__file is not None
        return self.__file

    def get_state(self) -> Dict:
        return {
            "name": self.__name,
            "size": self.__size,
            "written": self.__written,
        }

    async def open(self) -> "MsdImageWriter":
        assert self.__file is None
        get_logger(1).info("Writing %r image (%d bytes) to MSD ...", self.__name, self.__size)
        self.__file = await aiofiles.open(self.__path, mode="w+b", buffering=0)  # type: ignore
        return self

    async def write(self, chunk: bytes) -> int:
        assert self.__file is not None

        await self.__file.write(chunk)  # type: ignore
        self.__written += len(chunk)

        self.__unsynced += len(chunk)
        if self.__unsynced >= self.__sync:
            await aiofs.afile_sync(self.__file)
            self.__unsynced = 0

        return self.__written

    async def close(self) -> None:
        assert self.__file is not None
        if self.__written == self.__size:
            (log, result) = (get_logger().info, "OK")
        elif self.__written < self.__size:
            (log, result) = (get_logger().error, "INCOMPLETE")
        else:  # written > size
            (log, result) = (get_logger().warning, "OVERFLOW")
        log("Written %d of %d bytes to MSD image %r: %s", self.__written, self.__size, self.__name, result)
        await aiofs.afile_sync(self.__file)
        await self.__file.close()  # type: ignore


# =====
def get_msd_class(name: str) -> Type[BaseMsd]:
    return get_plugin_class("msd", name)  # type: ignore
