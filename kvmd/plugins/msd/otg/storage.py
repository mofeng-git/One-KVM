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

from ....logging import get_logger


# =====
@dataclasses.dataclass(frozen=True)
class Image:
    name: str
    path: str

    complete: bool = dataclasses.field(compare=False)
    in_storage: bool = dataclasses.field(compare=False)

    size: int = dataclasses.field(default=0, compare=False)
    mod_ts: float = dataclasses.field(default=0, compare=False)

    def exists(self) -> bool:
        return os.path.exists(self.path)

    def __post_init__(self) -> None:
        try:
            st = os.stat(self.path)
        except Exception:
            pass
        else:
            object.__setattr__(self, "size", st.st_size)
            object.__setattr__(self, "mod_ts", st.st_mtime)


@dataclasses.dataclass(frozen=True)
class StorageSpace:
    size: int
    free: int


class Storage:
    def __init__(self, path: str) -> None:
        self.__path = path
        self.__images_path = os.path.join(self.__path, "images")
        self.__meta_path = os.path.join(self.__path, "meta")

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
        complete = True
        in_storage = (os.path.dirname(path) == self.__images_path)
        if in_storage:
            complete = os.path.exists(os.path.join(self.__meta_path, name + ".complete"))
        return Image(name, path, complete, in_storage)

    def remove_image(self, image: Image, fatal: bool) -> None:
        assert image.in_storage
        try:
            os.remove(image.path)
        except FileNotFoundError:
            pass
        except Exception:
            if fatal:
                raise
        self.set_image_complete(image, False)

    def set_image_complete(self, image: Image, flag: bool) -> None:
        assert image.in_storage
        path = os.path.join(self.__meta_path, image.name + ".complete")
        if flag:
            open(path, "w").close()  # pylint: disable=consider-using-with
        else:
            try:
                os.remove(path)
            except FileNotFoundError:
                pass

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
