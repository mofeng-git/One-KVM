# ========================================================================== #
#                                                                            #
#    KVMD - The main PiKVM daemon.                                           #
#                                                                            #
#    Copyright (C) 2018-2024  Maxim Devaev <mdevaev@gmail.com>               #
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


import errno
import argparse

from ...validators.basic import valid_bool
from ...validators.basic import valid_int_f0
from ...validators.os import valid_abs_path

from ... import usb

from .. import init


# =====
def _get_param_path(gadget: str, instance: int, param: str) -> str:
    return usb.get_gadget_path(gadget, usb.G_FUNCTIONS, f"mass_storage.usb{instance}/lun.0", param)


def _get_param(gadget: str, instance: int, param: str) -> str:
    with open(_get_param_path(gadget, instance, param)) as file:
        return file.read().strip()


def _set_param(gadget: str, instance: int, param: str, value: str) -> None:
    try:
        with open(_get_param_path(gadget, instance, param), "w") as file:
            file.write(value + "\n")
    except OSError as ex:
        if ex.errno == errno.EBUSY:
            raise SystemExit(f"Can't change {param!r} value because device is locked: {ex}")
        raise


# =====
def main(argv: (list[str] | None)=None) -> None:
    (parent_parser, argv, config) = init(
        add_help=False,
        cli_logging=True,
        argv=argv,
        load_msd=True,
    )
    parser = argparse.ArgumentParser(
        prog="kvmd-otgmsd",
        description="KVMD OTG-MSD low-level hand tool",
        parents=[parent_parser],
    )
    parser.add_argument("-i", "--instance", default=0, type=valid_int_f0,
                        metavar="<N>", help="Drive instance (0 for KVMD drive)")
    parser.add_argument("--set-cdrom", default=None, type=valid_bool,
                        metavar="<1|0|yes|no>", help="Set CD-ROM flag")
    parser.add_argument("--set-rw", default=None, type=valid_bool,
                        metavar="<1|0|yes|no>", help="Set RW flag")
    parser.add_argument("--set-image", default=None, type=valid_abs_path,
                        metavar="<path>", help="Set the image file")
    parser.add_argument("--eject", action="store_true",
                        help="Eject the image")
    parser.add_argument("--unlock", action="store_true",
                        help="Does nothing, just for backward compatibility")
    options = parser.parse_args(argv[1:])

    if config.kvmd.msd.type != "otg":
        raise SystemExit(f"Error: KVMD MSD not using 'otg'"
                         f" (now configured {config.kvmd.msd.type!r})")

    set_param = (lambda param, value: _set_param(config.otg.gadget, options.instance, param, value))
    get_param = (lambda param: _get_param(config.otg.gadget, options.instance, param))

    if options.eject:
        set_param("forced_eject", "")

    if options.set_cdrom is not None:
        set_param("cdrom", str(int(options.set_cdrom)))

    if options.set_rw is not None:
        set_param("ro", str(int(not options.set_rw)))

    if options.set_image:
        # if not os.path.isfile(options.set_image):
        #     raise SystemExit(f"Not a file: {options.set_image}")
        set_param("file", options.set_image)

    print("Image file: ", (get_param("file") or "<none>"))
    print("CD-ROM flag:", ("yes" if int(get_param("cdrom")) else "no"))
    print("RW flag:    ", ("no" if int(get_param("ro")) else "yes"))
