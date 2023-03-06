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
import dataclasses

from typing import Generator
from typing import Optional

from ....logging import get_logger

from .... import aiohelpers

from .. import MsdError


# =====
@dataclasses.dataclass(frozen=True)
class _Image:
    name: str
    path: str
    storage: Optional["Storage"] = dataclasses.field(compare=False)

    in_storage: bool = dataclasses.field(init=False)

    complete: bool = dataclasses.field(init=False, compare=False)
    size: int = dataclasses.field(init=False, compare=False)
    mod_ts: float = dataclasses.field(init=False, compare=False)


class Image(_Image):
    @property
    def in_storage(self) -> bool:
        return (self.storage is not None)

    @property
    def complete(self) -> bool:
        if self.storage is not None:
            return os.path.exists(self.__get_complete_path())
        return True

    @property
    def size(self) -> int:
        try:
            return os.stat(self.path).st_size
        except Exception:
            return 0

    @property
    def mod_ts(self) -> float:
        try:
            return os.stat(self.path).st_mtime
        except Exception:
            return 0.0

    def exists(self) -> bool:
        return os.path.exists(self.path)

    async def remount_rw(self, rw: bool, fatal: bool=True) -> None:
        assert self.storage
        if self.storage._is_mounted(self):  # pylint: disable=protected-access
            await self.storage.remount_rw(rw, fatal)

    def remove(self, fatal: bool) -> None:
        assert self.storage is not None
        try:
            os.remove(self.path)
        except FileNotFoundError:
            pass
        except Exception:
            if fatal:
                raise
        self.set_complete(False)

    def set_complete(self, flag: bool) -> None:
        assert self.storage is not None
        path = self.__get_complete_path()
        if flag:
            open(path, "w").close()  # pylint: disable=consider-using-with
        else:
            try:
                os.remove(path)
            except FileNotFoundError:
                pass

    def __get_complete_path(self) -> str:
        return os.path.join(
            os.path.dirname(self.path),
            ".__" + os.path.basename(self.path) + ".complete",
        )


@dataclasses.dataclass(frozen=True)
class StorageSpace:
    size: int
    free: int


class Storage:
    def __init__(self, path: str, remount_cmd: list[str]) -> None:
        self.__path = path
        self.__remount_cmd = remount_cmd

    def get_watchable_paths(self) -> list[str]:
        paths: list[str] = []
        for (root_path, dirs, _) in os.walk(self.__path):
            dirs[:] = list(self.__filter(dirs))
            paths.append(root_path)
        return paths

    def get_images(self) -> dict[str, Image]:
        images: dict[str, Image] = {}
        for (root_path, dirs, files) in os.walk(self.__path):
            dirs[:] = list(self.__filter(dirs))
            for file in self.__filter(files):
                name = os.path.relpath(os.path.join(root_path, file), self.__path)
                images[name] = self.get_image_by_name(name)
        return images

    def __filter(self, items: list[str]) -> Generator[str, None, None]:
        for item in sorted(map(str.strip, items)):
            if not item.startswith(".") and item != "lost+found":
                yield item

    def get_image_by_name(self, name: str) -> Image:
        assert name
        path = os.path.join(self.__path, name)
        return self.__get_image(name, path, True)

    def get_image_by_path(self, path: str) -> Image:
        assert path
        in_storage = (os.path.commonpath([self.__path, path]) == self.__path)
        if in_storage:
            name = os.path.relpath(path, self.__path)
        else:
            name = os.path.basename(path)
        return self.__get_image(name, path, in_storage)

    def __get_image(self, name: str, path: str, in_storage: bool) -> Image:
        assert name
        assert path
        return Image(name, path, (self if in_storage else None))

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

    def _is_mounted(self, image: Image) -> bool:
        path = image.path
        while not os.path.ismount(path):
            path = os.path.dirname(path)
        return (path == self.__path)

    async def remount_rw(self, rw: bool, fatal: bool=True) -> None:
        if not (await aiohelpers.remount("MSD", self.__remount_cmd, rw)):
            if fatal:
                raise MsdError("Can't execute remount helper")
