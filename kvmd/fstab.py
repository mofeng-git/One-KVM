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


import re
import dataclasses


# =====
class PartitionType:
    MSD = "otgmsd"
    PST = "pst"
    ALL = (
        MSD,
        PST,
    )


@dataclasses.dataclass(frozen=True)
class Partition:
    mount_path: str
    root_path: str
    user: str


def find_partition(part_type: str) -> Partition:
    assert part_type in PartitionType.ALL
    fstab_path = "/etc/fstab"
    with open(fstab_path) as file:
        for line in file.read().split("\n"):
            line = line.strip()
            if line and not line.startswith("#"):
                parts = line.split()
                if len(parts) == 6:
                    options = dict(re.findall(r"X-kvmd\.%s-(root|user)(?:=([^,]+))?" % (part_type), parts[3]))
                    if options:
                        return Partition(
                            mount_path=parts[1],
                            root_path=(options.get("root", "") or parts[1]),
                            user=options.get("user", ""),
                        )
    raise RuntimeError(f"Can't find {part_type!r} mountpoint in {fstab_path}")
