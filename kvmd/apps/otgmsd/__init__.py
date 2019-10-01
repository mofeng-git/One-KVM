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


import os
import signal
import errno
import argparse

import psutil


# =====
def _set_param(gadget: str, param: str, value: str) -> None:
    param_path = os.path.join(
        "/sys/kernel/config/usb_gadget",
        gadget,
        "functions/mass_storage.usb0/lun.0",
        param,
    )
    try:
        with open(param_path, "w") as param_file:
            param_file.write(value + "\n")
    except OSError as err:
        if err.errno == errno.EBUSY:
            raise SystemExit(f"Can't change {param!r} value because device is locked: {err}")
        raise


def _reset_msd() -> None:
    # https://github.com/torvalds/linux/blob/3039fad/drivers/usb/gadget/function/f_mass_storage.c#L2924
    found = False
    for proc in psutil.process_iter():
        attrs = proc.as_dict(attrs=["name", "exe", "pid"])
        if attrs.get("name") == "file-storage" and not attrs.get("exe"):
            try:
                proc.send_signal(signal.SIGUSR1)
                found = True
            except Exception as err:
                SystemExit(f"Can't send SIGUSR1 to MSD kernel thread with pid={attrs['pid']}: {err}")
    if not found:
        raise SystemExit("Can't find MSD kernel thread")


# =====
def main() -> None:
    parser = argparse.ArgumentParser(description="KVMD OTG MSD Helper")
    parser.add_argument("--reset", action="store_true", help="Send SIGUSR1 to MSD kernel thread")
    parser.add_argument("--set-cdrom", dest="cdrom", default=None, choices=["0", "1"], help="Set CD-ROM flag")
    parser.add_argument("--set-ro", dest="ro", default=None, choices=["0", "1"], help="Set read-only flag")
    parser.add_argument("--set-image", dest="image_path", default=None, help="Change image path (or eject for the empty)")
    parser.add_argument("--gadget", default="kvmd", help="USB gadget name")
    options = parser.parse_args()

    if options.reset:
        _reset_msd()

    if options.cdrom is not None:
        _set_param(options.gadget, "cdrom", options.cdrom)

    if options.ro is not None:
        _set_param(options.gadget, "ro", options.ro)

    if options.image_path is not None:
        _set_param(options.gadget, "file", options.image_path)
