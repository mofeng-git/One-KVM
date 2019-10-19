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

from typing import List
from typing import Optional

import psutil
import yaml

from ...validators.kvm import valid_msd_image_name

from .. import init


# =====
def _make_param_path(gadget: str, param: str) -> str:
    return os.path.join(
        "/sys/kernel/config/usb_gadget",
        gadget,
        "functions/mass_storage.usb0/lun.0",
        param,
    )


def _get_param(gadget: str, param: str) -> str:
    with open(_make_param_path(gadget, param)) as param_file:
        return param_file.read().strip()


def _set_param(gadget: str, param: str, value: str) -> None:
    try:
        with open(_make_param_path(gadget, param), "w") as param_file:
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
                raise SystemExit(f"Can't send SIGUSR1 to MSD kernel thread with pid={attrs['pid']}: {err}")
    if not found:
        raise SystemExit("Can't find MSD kernel thread")


# =====
def main(argv: Optional[List[str]]=None) -> None:
    (parent_parser, argv, config) = init(
        add_help=False,
        argv=argv,
        load_msd=True,
    )
    parser = argparse.ArgumentParser(
        prog="kvmd-otg-msd",
        description="KVMD OTG MSD Helper",
        parents=[parent_parser],
    )
    parser.add_argument("--reset", action="store_true", help="Send SIGUSR1 to MSD kernel thread")
    parser.add_argument("--set-cdrom", default=None, choices=["0", "1"], help="Set CD-ROM flag")
    parser.add_argument("--set-ro", default=None, choices=["0", "1"], help="Set read-only flag")
    parser.add_argument("--set-image", default=None, type=valid_msd_image_name, help="Change the image")
    parser.add_argument("--eject", action="store_true", help="Eject the image")
    options = parser.parse_args(argv[1:])

    if config.kvmd.msd.type != "otg":
        raise SystemExit(f"Error: KVMD MSD not using 'otg'"
                         f" (now configured {config.kvmd.msd.type!r})")

    if options.reset:
        _reset_msd()

    if options.eject:
        _set_param(config.otg.gadget, "file", "")

    if options.set_cdrom is not None:
        _set_param(config.otg.gadget, "cdrom", options.set_cdrom)

    if options.set_ro is not None:
        _set_param(config.otg.gadget, "ro", options.set_ro)

    if options.set_image:
        path = os.path.join(config.kvmd.msd.storage, "images", options.set_image)
        if not os.path.isfile(path):
            raise SystemExit(f"Can't find image {path!r}")
        _set_param(config.otg.gadget, "file", path)

    print(yaml.dump({  # type: ignore
        name: _get_param(config.otg.gadget, param)
        for (param, name) in [
            ("file", "image"),
            ("cdrom", "cdrom"),
            ("ro", "ro"),
        ]
    }, default_flow_style=False, sort_keys=False), end="")
