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
class _ImageDc:
    name: str
    path: str
    in_storage: bool = dataclasses.field(init=False, compare=False)
    complete: bool = dataclasses.field(init=False, compare=False)
    removable: bool = dataclasses.field(init=False, compare=False)
    size: int = dataclasses.field(init=False, compare=False)
    mod_ts: float = dataclasses.field(init=False, compare=False)


class Image(_ImageDc):
    def __init__(self, name: str, path: str, storage: Optional["Storage"]) -> None:
        super().__init__(name, path)
        self.__storage = storage
        (self.__dir_path, file_name) = os.path.split(path)
        self.__incomplete_path = os.path.join(self.__dir_path, f".__{file_name}.incomplete")
        self.__adopted = False

    async def _reload(self) -> None:  # Only for Storage() and set_complete()
        # adopted используется в последующих проверках
        self.__adopted = await aiotools.run_async(self.__is_adopted)
        complete = await self.__is_complete()
        removable = await self.__is_removable()
        (size, mod_ts) = await self.__get_stat()
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
            return (not (await aiofiles.os.path.exists(self.__incomplete_path)))
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
        removed = False
        try:
            await aiofiles.os.remove(self.path)
            removed = True
            self.__storage.images.pop(self.name, None)
        except FileNotFoundError:
            pass
        except Exception:
            if fatal:
                raise
        finally:
            # Удаляем .incomplete вместе с файлом
            if removed:
                await self.set_complete(True)

    async def set_complete(self, flag: bool) -> None:
        assert self.__storage
        if flag:
            try:
                await aiofiles.os.remove(self.__incomplete_path)
            except FileNotFoundError:
                pass
        else:
            async with aiofiles.open(self.__incomplete_path, "w"):
                pass
        await self._reload()


# =====
@dataclasses.dataclass(frozen=True)
class _PartDc:
    name: str
    size: int = dataclasses.field(init=False, compare=False)
    free: int = dataclasses.field(init=False, compare=False)
    writable: bool = dataclasses.field(init=False, compare=False)


class _Part(_PartDc):
    def __init__(self, name: str, path: str) -> None:
        super().__init__(name)
        self.__path = path

    async def _reload(self) -> None:  # Only for Storage()
        st = await aiotools.run_async(os.statvfs, self.__path)
        if self.name == "":
            writable = True
        else:
            writable = await aiofiles.os.access(self.__path, os.W_OK)  # type: ignore
        object.__setattr__(self, "size", st.f_blocks * st.f_frsize)
        object.__setattr__(self, "free", st.f_bavail * st.f_frsize)
        object.__setattr__(self, "writable", writable)


# =====
@dataclasses.dataclass(frozen=True, eq=False)
class _StorageDc:
    size: int = dataclasses.field(init=False)
    free: int = dataclasses.field(init=False)
    images: dict[str, Image] = dataclasses.field(init=False)
    parts: dict[str, _Part] = dataclasses.field(init=False)


class Storage(_StorageDc):
    def __init__(self, path: str, remount_cmd: list[str]) -> None:
        super().__init__()
        self.__path = path
        self.__remount_cmd = remount_cmd

        self.__watchable_paths: (list[str] | None) = None
        self.__images: (dict[str, Image] | None) = None
        self.__parts: (dict[str, _Part] | None) = None

    @property
    def size(self) -> int:  # API Legacy
        assert self.__parts is not None
        return self.__parts[""].size

    @property
    def free(self) -> int:  # API Legacy
        assert self.__parts is not None
        return self.__parts[""].free

    @property
    def images(self) -> dict[str, Image]:
        assert self.__images is not None
        return self.__images

    @property
    def parts(self) -> dict[str, _Part]:
        assert self.__parts is not None
        return self.__parts

    async def reload(self) -> None:
        self.__watchable_paths = None
        self.__images = {}

        watchable_paths: list[str] = []
        images: dict[str, Image] = {}
        parts: dict[str, _Part] = {}
        for (root_path, is_part, files) in (await aiotools.run_async(self.__walk)):
            watchable_paths.append(root_path)
            for path in files:
                name = self.__make_relative_name(path)
                images[name] = await self.make_image_by_name(name)
            if is_part:
                name = self.__make_relative_name(root_path, dot_to_empty=True)
                part = _Part(name, root_path)
                await part._reload()  # pylint: disable=protected-access
                parts[name] = part

        self.__watchable_paths = watchable_paths
        self.__images = images
        self.__parts = parts

    async def reload_parts_info(self) -> None:
        await asyncio.gather(*[part._reload() for part in self.parts.values()])  # pylint: disable=protected-access

    def get_watchable_paths(self) -> list[str]:
        assert self.__watchable_paths is not None
        return list(self.__watchable_paths)

    def __walk(self) -> list[tuple[str, bool, list[str]]]:
        return list(self.__inner_walk(self.__path))

    def __inner_walk(self, root_path: str) -> Generator[tuple[str, bool, list[str]], None, None]:
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
        yield (root_path, (root_path == self.__path or os.path.ismount(root_path)), files)

    def __make_relative_name(self, path: str, dot_to_empty: bool=False) -> str:
        name = os.path.relpath(path, self.__path)
        assert name
        if dot_to_empty and name == ".":
            name = ""
        assert not name.startswith(".")
        return name

    # =====

    async def make_image_by_name(self, name: str) -> Image:
        assert name
        path = os.path.join(self.__path, name)
        return (await self.__make_image(name, path, True))

    async def make_image_by_path(self, path: str) -> Image:
        assert path
        in_storage = (os.path.commonpath([self.__path, path]) == self.__path)
        if in_storage:
            name = self.__make_relative_name(path)
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
