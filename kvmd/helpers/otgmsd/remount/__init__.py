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


import sys
import os
import re
import shutil
import dataclasses
import subprocess


# ====
_MOUNT_PATH = "/bin/mount"
_FSTAB_PATH = "/etc/fstab"


# =====
@dataclasses.dataclass(frozen=True)
class _Storage:
    mount_path: str
    root_path: str
    user: str


# =====
def _log(msg: str) -> None:
    print(msg, file=sys.stderr)


def _find_storage() -> _Storage:
    with open(_FSTAB_PATH) as fstab_file:
        for line in fstab_file.read().split("\n"):
            line = line.strip()
            if line and not line.startswith("#"):
                parts = line.split()
                if len(parts) == 6:
                    options = dict(re.findall(r"X-kvmd\.otgmsd-(root|user)=([^,]+)", parts[3]))
                    if options:
                        return _Storage(
                            mount_path=parts[1],
                            root_path=options.get("root", ""),
                            user=options.get("user", ""),
                        )
    raise RuntimeError(f"Can't find MSD mountpoint in {_FSTAB_PATH}")


def _remount(path: str, rw: bool) -> None:
    mode = ("rw" if rw else "ro")
    _log(f"Remounting {path} to {mode.upper()}-mode ...")
    try:
        subprocess.check_call([_MOUNT_PATH, "--options", f"remount,{mode}", path])
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

    rw = (sys.argv[1] == "rw")

    storage = _find_storage()
    _remount(storage.mount_path, rw)
    if rw:
        if storage.root_path:
            for name in ["images", "meta"]:
                path = os.path.join(storage.root_path, name)
                _mkdir(path)
                if storage.user:
                    _chown(path, storage.user)
