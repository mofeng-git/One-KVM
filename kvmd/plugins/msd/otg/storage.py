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

    def get_images(self) -> list[str]:
        images: list[str] = []
        for name in os.listdir(self.__images_path):
            path = os.path.join(self.__images_path, name)
            if os.path.exists(path):
                try:
                    if os.path.getsize(path) >= 0:
                        images.append(name)
                except Exception:
                    pass
        return images

    def get_image_path(self, name: str) -> str:
        return os.path.join(self.__images_path, name)

    def is_image_path_in_storage(self, path: str) -> bool:
        return (os.path.dirname(path) == self.__images_path)

    def is_image_complete(self, name: str) -> bool:
        return os.path.exists(os.path.join(self.__meta_path, name + ".complete"))

    def set_image_complete(self, name: str, flag: bool) -> None:
        path = os.path.join(self.__meta_path, name + ".complete")
        if flag:
            open(path, "w").close()  # pylint: disable=consider-using-with
        else:
            if os.path.exists(path):
                os.remove(path)

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
