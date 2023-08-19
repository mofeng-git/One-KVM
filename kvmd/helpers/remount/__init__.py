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


import sys
import os
import pwd
import shutil
import subprocess

from os.path import join  # pylint: disable=ungrouped-imports
from os.path import exists  # pylint: disable=ungrouped-imports

from ...fstab import Partition
from ...fstab import find_msd
from ...fstab import find_pst


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
    if not exists(path):
        _log(f"MKDIR --- {path}")
        try:
            os.mkdir(path)
        except Exception as err:
            raise SystemExit(f"Can't create directory: {err}")


def _rmtree(path: str) -> None:
    if exists(path):
        _log(f"RMALL --- {path}")
        try:
            shutil.rmtree(path)
        except Exception as err:
            raise SystemExit(f"Can't remove directory: {err}")


def _rm(path: str) -> None:
    if exists(path):
        _log(f"RM    --- {path}")
        try:
            os.remove(path)
        except Exception as err:
            raise SystemExit(f"Can't remove file: {err}")


def _move(src: str, dest: str) -> None:
    _log(f"MOVE  --- {src} --> {dest}")
    try:
        os.rename(src, dest)
    except Exception as err:
        raise SystemExit(f"Can't move file: {err}")


def _chown(path: str, user: str) -> None:
    if pwd.getpwuid(os.stat(path).st_uid).pw_name != user:
        _log(f"CHOWN --- {user} - {path}")
        try:
            shutil.chown(path, user)
        except Exception as err:
            raise SystemExit(f"Can't change ownership: {err}")


# =====
def _fix_msd(part: Partition) -> None:
    # First images migration
    images_path = join(part.root_path, "images")
    meta_path = join(part.root_path, "meta")
    if exists(images_path) and exists(meta_path):
        for name in os.listdir(images_path):
            _move(join(images_path, name), os.path.join(part.root_path, name))
            if not exists(join(meta_path, f"{name}.complete")):
                open(os.path.join(part.root_path, f".__{name}.incomplete")).close()  # pylint: disable=consider-using-with
        _rmtree(images_path)
        _rmtree(meta_path)

    # Second images migration
    for name in os.listdir(part.root_path):
        if name.startswith(".__") and name.endswith(".complete"):
            _rm(join(part.root_path, name))

    if part.user:
        _chown(part.root_path, part.user)


def _fix_pst(part: Partition) -> None:
    path = os.path.join(part.root_path, "data")
    _mkdir(path)
    if part.user:
        _chown(path, part.user)


# =====
def main() -> None:
    if len(sys.argv) != 2 or sys.argv[1] not in ["ro", "rw"]:
        raise SystemExit(f"Usage: {sys.argv[0]} [ro|rw]")

    finder = None
    fix = None
    app = os.path.basename(sys.argv[0])
    if app == "kvmd-helper-otgmsd-remount":
        finder = find_msd
        fix = _fix_msd
    elif app == "kvmd-helper-pst-remount":
        finder = find_pst
        fix = _fix_pst
    else:
        raise SystemExit("Unknown application target")

    rw = (sys.argv[1] == "rw")

    assert finder is not None
    part = finder()
    _remount(part.mount_path, rw)
    if rw and part.root_path:
        fix(part)
    _log(f"Storage in the {'RW' if rw else 'RO'}-mode now")
