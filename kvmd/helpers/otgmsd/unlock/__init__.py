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
import signal

import psutil


# =====
_PROCESS_NAME = "file-storage"


# =====
def _log(msg: str) -> None:
    print(msg, file=sys.stderr)


def _unlock() -> None:
    # https://github.com/torvalds/linux/blob/3039fad/drivers/usb/gadget/function/f_mass_storage.c#L2924
    found = False
    for proc in psutil.process_iter():
        attrs = proc.as_dict(attrs=["name", "exe", "pid"])
        if attrs.get("name") == _PROCESS_NAME and not attrs.get("exe"):
            _log(f"Sending SIGUSR1 to MSD {_PROCESS_NAME!r} kernel thread with pid={attrs['pid']} ...")
            try:
                proc.send_signal(signal.SIGUSR1)
                found = True
            except Exception as err:
                raise SystemExit(f"Can't send SIGUSR1 to MSD kernel thread with pid={attrs['pid']}: {err}")
    if not found:
        raise SystemExit(f"Can't find MSD kernel thread {_PROCESS_NAME!r}")


# =====
def main() -> None:
    if len(sys.argv) != 2 or sys.argv[1] != "unlock":
        raise SystemExit(f"Usage: {sys.argv[0]} [unlock]")
    _unlock()
