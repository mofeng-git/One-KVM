# ========================================================================== #
#                                                                            #
#    KVMD - The main Pi-KVM daemon.                                          #
#                                                                            #
#    Copyright (C) 2018  Maxim Devaev <mdevaev@gmail.com>                    #
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
import subprocess


# ====
_MOUNT_PATH = "/bin/mount"
_FSTAB_PATH = "/etc/fstab"
_OPTION = "X-kvmd.otg-msd"


# =====
def _find_mountpoint() -> str:
    with open(_FSTAB_PATH) as fstab_file:
        for line in fstab_file.read().split("\n"):
            line = line.strip()
            if line and not line.startswith("#"):
                parts = line.split()
                if len(parts) == 6:
                    options = parts[3].split(",")
                    if _OPTION in options:
                        return parts[1]
    raise SystemExit(f"Can't find {_OPTION!r} mountpoint in {_FSTAB_PATH}")


def _remount(path: str, ro: bool) -> None:
    try:
        subprocess.check_call([
            _MOUNT_PATH,
            "--options",
            f"remount,{'ro' if ro else 'rw'}",
            path,
        ])
    except subprocess.CalledProcessError as err:
        raise SystemExit(str(err)) from None


# =====
def main() -> None:
    if len(sys.argv) != 2 or sys.argv[1] not in ["ro", "rw"]:
        raise SystemExit(f"This program will remount a first volume marked by {_OPTION!r} option in {_FSTAB_PATH}\n\n"
                         f"Usage: python -m kvmd.helpers.otgmsd.remount [-h|--help|ro|rw]")
    _remount(_find_mountpoint(), (sys.argv[1] == "ro"))
