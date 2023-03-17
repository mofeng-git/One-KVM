# ========================================================================== #
#                                                                            #
#    KVMD - The main PiKVM daemon.                                           #
#                                                                            #
#    Copyright (C) 2018-2022  Maxim Devaev <mdevaev@gmail.com>               #
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
import asyncio
import operator
import dataclasses

from typing import Generator
from typing import Optional

import aiofiles
import aiofiles.os

from ....logging import get_logger

from .... import aiotools
from .... import aiohelpers

from .. import MsdError


# =====
@dataclasses.dataclass(frozen=True)
class _Image:
    name: str
    path: str
    in_storage: bool = dataclasses.field(init=False, compare=False)
    complete: bool = dataclasses.field(init=False, compare=False)
    removable: bool = dataclasses.field(init=False, compare=False)
    size: int = dataclasses.field(init=False, compare=False)
    mod_ts: float = dataclasses.field(init=False, compare=False)


class Image(_Image):
    def __init__(self, name: str, path: str, storage: Optional["Storage"]) -> None:
        super().__init__(name, path)
        self.__storage = storage
        (self.__dir_path, file_name) = os.path.split(path)
        self.__complete_path = os.path.join(self.__dir_path, f".__{file_name}.complete")
        self.__adopted = False

    async def _update(self) -> None:
        # adopted используется в последующих проверках
        self.__adopted = await aiotools.run_async(self.__is_adopted)
        (complete, removable, (size, mod_ts)) = await asyncio.gather(
            self.__is_complete(),
            self.__is_removable(),
            self.__get_stat(),
        )
        object.__setattr__(self, "complete", complete)
        object.__setattr__(self, "removable", removable)
        object.__setattr__(self, "size", size)
        object.__setattr__(self, "mod_ts", mod_ts)

    def __is_adopted(self) -> bool:
        # True, если образ находится вне хранилища
        # или в другой точке монтирования под ним
        if self.__storage is None:
            return True
        path = self.path
        while not os.path.ismount(path):
            path = os.path.dirname(path)
        return (self.__storage._get_root_path() != path)

    async def __is_complete(self) -> bool:
        if self.__storage:
            return (await aiofiles.os.path.exists(self.__complete_path))
        return True

    async def __is_removable(self) -> bool:
        if not self.__storage:
            return False
        if not self.__adopted:
            return True
        return (await aiofiles.os.access(self.__dir_path, os.W_OK))  # type: ignore

    async def __get_stat(self) -> tuple[int, float]:
        try:
            st = (await aiofiles.os.stat(self.path))
            return (st.st_size, st.st_mtime)
        except Exception:
            return (0, 0.0)

    # =====

    @property
    def in_storage(self) -> bool:
        return bool(self.__storage)

    async def exists(self) -> bool:
        return (await aiofiles.os.path.exists(self.path))

    async def remount_rw(self, rw: bool, fatal: bool=True) -> None:
        assert self.__storage
        if not self.__adopted:
            await self.__storage.remount_rw(rw, fatal)

    async def remove(self, fatal: bool) -> None:
        assert self.__storage
        try:
            await aiofiles.os.remove(self.path)
        except FileNotFoundError:
            pass
        except Exception:
            if fatal:
                raise
        await self.set_complete(False)

    async def set_complete(self, flag: bool) -> None:
        assert self.__storage
        if flag:
            async with aiofiles.open(self.__complete_path, "w"):
                pass
        else:
            try:
                await aiofiles.os.remove(self.__complete_path)
            except FileNotFoundError:
                pass
        await self._update()


@dataclasses.dataclass(frozen=True)
class StorageSpace:
    size: int
    free: int


class Storage:
    def __init__(self, path: str, remount_cmd: list[str]) -> None:
        self.__path = path
        self.__remount_cmd = remount_cmd

    def _get_root_path(self) -> str:
        return self.__path

    async def get_watchable_paths(self) -> list[str]:
        return (await aiotools.run_async(self.__inner_get_watchable_paths))

    async def get_images(self) -> dict[str, Image]:
        return {
            name: (await self.make_image_by_name(name))
            for name in (await aiotools.run_async(self.__inner_get_images))
        }

    def __inner_get_watchable_paths(self) -> list[str]:
        return list(map(operator.itemgetter(0), self.__walk(with_files=False)))

    def __inner_get_images(self) -> list[str]:
        return [
            os.path.relpath(path, self.__path)  # == name
            for (_, files) in self.__walk(with_files=True)
            for path in files
        ]

    def __walk(self, with_files: bool, root_path: (str | None)=None) -> Generator[tuple[str, list[str]], None, None]:
        if root_path is None:
            root_path = self.__path
        files: list[str] = []
        with os.scandir(root_path) as dir_iter:
            for item in sorted(dir_iter, key=operator.attrgetter("name")):
                if item.name.startswith(".") or item.name == "lost+found":
                    continue
                try:
                    if item.is_dir(follow_symlinks=False):
                        item.stat()  # Проверяем, не сдохла ли смонтированная NFS
                        yield from self.__walk(with_files, item.path)
                    elif with_files and item.is_file(follow_symlinks=False):
                        files.append(item.path)
                except Exception:
                    pass
        yield (root_path, files)

    # =====

    async def make_image_by_name(self, name: str) -> Image:
        assert name
        path = os.path.join(self.__path, name)
        return (await self.__get_image(name, path, True))

    async def make_image_by_path(self, path: str) -> Image:
        assert path
        in_storage = (os.path.commonpath([self.__path, path]) == self.__path)
        if in_storage:
            name = os.path.relpath(path, self.__path)
        else:
            name = os.path.basename(path)
        return (await self.__get_image(name, path, in_storage))

    async def __get_image(self, name: str, path: str, in_storage: bool) -> Image:
        assert name
        assert path
        image = Image(name, path, (self if in_storage else None))
        await image._update()  # pylint: disable=protected-access
        return image

    # =====

    def get_space(self, fatal: bool) -> (StorageSpace | None):
        try:
            st = os.statvfs(self.__path)
        except Exception as err:
            if fatal:
                raise
            get_logger().warning("Can't get free space of filesystem %s: %s", self.__path, err)
            return None
        return StorageSpace(
            size=(st.f_blocks * st.f_frsize),
            free=(st.f_bavail * st.f_frsize),
        )

    async def remount_rw(self, rw: bool, fatal: bool=True) -> None:
        if not (await aiohelpers.remount("MSD", self.__remount_cmd, rw)):
            if fatal:
                raise MsdError("Can't execute remount helper")
