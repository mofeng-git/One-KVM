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

    async def _reload(self) -> None:  # Only for Storage() and set_complete()
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
        return (self.__storage._get_root_path() != path)  # pylint: disable=protected-access

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
            self.__storage.images.pop(self.name, None)
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
        await self._reload()


@dataclasses.dataclass(frozen=True, eq=False)
class _Storage:
    size: int
    free: int
    images: dict[str, Image] = dataclasses.field(init=False)


class Storage(_Storage):
    def __init__(self, path: str, remount_cmd: list[str]) -> None:
        super().__init__(0, 0)
        self.__path = path
        self.__remount_cmd = remount_cmd
        self.__watchable_paths: (list[str] | None) = None
        self.__images: (dict[str, Image] | None) = None

    @property
    def images(self) -> dict[str, Image]:
        assert self.__watchable_paths is not None
        assert self.__images is not None
        return self.__images

    async def reload(self) -> None:
        self.__watchable_paths = None
        self.__images = {}

        watchable_paths: list[str] = []
        images: dict[str, Image] = {}
        for (root_path, files) in (await aiotools.run_async(self.__walk)):
            watchable_paths.append(root_path)
            for path in files:
                name = os.path.relpath(path, self.__path)
                images[name] = await self.make_image_by_name(name)

        await self.reload_size_only()

        self.__watchable_paths = watchable_paths
        self.__images = images

    async def reload_size_only(self) -> None:
        st = os.statvfs(self.__path)  # FIXME
        object.__setattr__(self, "size", st.f_blocks * st.f_frsize)
        object.__setattr__(self, "free", st.f_bavail * st.f_frsize)

    def get_watchable_paths(self) -> list[str]:
        assert self.__watchable_paths is not None
        return list(self.__watchable_paths)

    def __walk(self) -> list[tuple[str, list[str]]]:
        return list(self.__inner_walk(self.__path))

    def __inner_walk(self, root_path: str) -> Generator[tuple[str, list[str]], None, None]:
        files: list[str] = []
        with os.scandir(root_path) as dir_iter:
            for item in sorted(dir_iter, key=operator.attrgetter("name")):
                if item.name.startswith(".") or item.name == "lost+found":
                    continue
                try:
                    if item.is_dir(follow_symlinks=False):
                        item.stat()  # Проверяем, не сдохла ли смонтированная NFS
                        yield from self.__inner_walk(item.path)
                    elif item.is_file(follow_symlinks=False):
                        files.append(item.path)
                except Exception:
                    pass
        yield (root_path, files)

    # =====

    async def make_image_by_name(self, name: str) -> Image:
        assert name
        path = os.path.join(self.__path, name)
        return (await self.__make_image(name, path, True))

    async def make_image_by_path(self, path: str) -> Image:
        assert path
        in_storage = (os.path.commonpath([self.__path, path]) == self.__path)
        if in_storage:
            name = os.path.relpath(path, self.__path)
        else:
            name = os.path.basename(path)
        return (await self.__make_image(name, path, in_storage))

    async def __make_image(self, name: str, path: str, in_storage: bool) -> Image:
        assert name
        assert path
        image = Image(name, path, (self if in_storage else None))
        await image._reload()  # pylint: disable=protected-access
        return image

    def _get_root_path(self) -> str:  # Only for Image()
        return self.__path

    # =====

    async def remount_rw(self, rw: bool, fatal: bool=True) -> None:
        if not (await aiohelpers.remount("MSD", self.__remount_cmd, rw)):
            if fatal:
                raise MsdError("Can't execute remount helper")
