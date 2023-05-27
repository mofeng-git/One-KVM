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
import re
import dataclasses

from . import env


# =====
@dataclasses.dataclass(frozen=True)
class Partition:
    mount_path: str
    root_path: str
    user: str


# =====
def find_msd() -> Partition:
    return _find_single("otgmsd")


def find_pst() -> Partition:
    return _find_single("pst")


# =====
def _find_single(part_type: str) -> Partition:
    parts = _find_partitions(part_type, True)
    if len(parts) == 0:
        raise RuntimeError(f"Can't find {part_type!r} mountpoint")
    return parts[0]


def _find_partitions(part_type: str, single: bool) -> list[Partition]:
    parts: list[Partition] = []
    with open(f"{env.ETC_PREFIX}/etc/fstab") as file:
        for line in file.read().split("\n"):
            line = line.strip()
            if line and not line.startswith("#"):
                fields = line.split()
                if len(fields) == 6:
                    options = dict(re.findall(r"X-kvmd\.%s-(root|user)(?:=([^,]+))?" % (part_type), fields[3]))
                    if options:
                        parts.append(Partition(
                            mount_path=os.path.normpath(fields[1]),
                            root_path=os.path.normpath(options.get("root", "") or fields[1]),
                            user=options.get("user", ""),
                        ))
                        if single:
                            break
    return parts
