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

from typing import Optional

from ....logging import get_logger


# =====
@dataclasses.dataclass(frozen=True)
class _Image:
    name: str
    path: str
    storage: Optional["Storage"] = dataclasses.field(compare=False)

    complete: bool = dataclasses.field(init=False, compare=False)
    in_storage: bool = dataclasses.field(init=False, compare=False)

    size: int = dataclasses.field(init=False, compare=False)
    mod_ts: float = dataclasses.field(init=False, compare=False)


class Image(_Image):
    @property
    def complete(self) -> bool:
        if self.storage is not None:
            return os.path.exists(self.storage._get_complete_path(self))  # pylint: disable=protected-access
        return True

    @property
    def in_storage(self) -> bool:
        return (self.storage is not None)

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
            return 0

    def exists(self) -> bool:
        return os.path.exists(self.path)

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
        path = self.storage._get_complete_path(self)  # pylint: disable=protected-access
        if flag:
            open(path, "w").close()  # pylint: disable=consider-using-with
        else:
            try:
                os.remove(path)
            except FileNotFoundError:
                pass


@dataclasses.dataclass(frozen=True)
class StorageSpace:
    size: int
    free: int


class Storage:
    def __init__(self, path: str) -> None:
        self.__path = path
        self.__images_path = os.path.join(self.__path, "images")
        self.__meta_path = os.path.join(self.__path, "meta")

    def _get_complete_path(self, image: Image) -> str:
        return os.path.join(self.__meta_path, image.name + ".complete")

    def get_watchable_paths(self) -> list[str]:
        return [self.__images_path, self.__meta_path]

    def get_images(self) -> dict[str, Image]:
        return {
            name: self.get_image_by_name(name)
            for name in os.listdir(self.__images_path)
        }

    def get_image_by_name(self, name: str) -> Image:
        assert name
        path = os.path.join(self.__images_path, name)
        return self.__get_image(name, path)

    def get_image_by_path(self, path: str) -> Image:
        assert path
        name = os.path.basename(path)
        return self.__get_image(name, path)

    def __get_image(self, name: str, path: str) -> Image:
        assert name
        assert path
        in_storage = (os.path.dirname(path) == self.__images_path)
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
