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
import dataclasses

from typing import Optional

from ....logging import get_logger


# =====
@dataclasses.dataclass(frozen=True)
class FsSpace:
    size: int
    free: int


# =====
def get_file_size(path: str) -> int:
    try:
        return os.path.getsize(path)
    except Exception as err:
        get_logger().warning("Can't get size of file %s: %s", path, err)
        return -1


def get_fs_space(path: str, fatal: bool) -> Optional[FsSpace]:
    try:
        st = os.statvfs(path)
    except Exception as err:
        if fatal:
            raise
        get_logger().warning("Can't get free space of filesystem %s: %s", path, err)
        return None
    return FsSpace(
        size=(st.f_blocks * st.f_frsize),
        free=(st.f_bavail * st.f_frsize),
    )
