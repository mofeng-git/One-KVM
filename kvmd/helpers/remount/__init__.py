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


import sys
import os
import shutil
import subprocess

from ... import fstab


# =====
def _log(msg: str) -> None:
    print(msg, file=sys.stderr)


def _remount(path: str, rw: bool) -> None:
    mode = ("rw" if rw else "ro")
    _log(f"Remounting {path} to {mode.upper()}-mode ...")
    try:
        subprocess.check_call(["/bin/mount", "--options", f"remount,{mode}", path])
    except subprocess.CalledProcessError as err:
        raise SystemExit(f"Can't remount: {err}")


def _mkdir(path: str) -> None:
    if not os.path.exists(path):
        _log(f"MKDIR --- {path}")
        try:
            os.mkdir(path)
        except Exception as err:
            raise SystemExit(f"Can't create directory: {err}")


def _chown(path: str, user: str) -> None:
    _log(f"CHOWN --- {user} - {path}")
    try:
        shutil.chown(path, user)
    except Exception as err:
        raise SystemExit(f"Can't change ownership: {err}")


# =====
def main() -> None:
    if len(sys.argv) != 2 or sys.argv[1] not in ["ro", "rw"]:
        raise SystemExit(f"Usage: {sys.argv[0]} [ro|rw]")

    part_type = ""
    dirs: list[str] = []
    app = os.path.basename(sys.argv[0])
    if app == "kvmd-helper-otgmsd-remount":
        part_type = fstab.PartitionType.MSD
        dirs = ["images", "meta"]
    elif app == "kvmd-helper-pst-remount":
        part_type = fstab.PartitionType.PST
        dirs = ["data"]
    else:
        raise SystemExit("Unknown application target")

    rw = (sys.argv[1] == "rw")

    part = fstab.find_partition(part_type)
    _remount(part.mount_path, rw)
    if rw and part.root_path:
        for name in dirs:
            path = os.path.join(part.root_path, name)
            _mkdir(path)
            if part.user:
                _chown(path, part.user)
    _log(f"Storage in the {'RW' if rw else 'RO'}-mode now")
