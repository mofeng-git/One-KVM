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


import time
import argparse

from os import listdir
from os import mkdir
from os import symlink
from os import rmdir
from os import unlink
from os.path import join

from typing import List
from typing import Optional

from ...yamlconf import Section

from ...validators import ValidatorError

from .. import init


# =====
def _write(path: str, text: str) -> None:
    with open(path, "w") as param_file:
        param_file.write(text)


def _find_udc(udc: str) -> str:
    udcs = sorted(listdir("/sys/class/udc"))
    if not udc:
        if len(udcs) == 0:
            raise RuntimeError("Can't find any UDC")
        udc = udcs[0]
    elif udc not in udcs:
        raise RuntimeError(f"Can't find selected UDC: {udc}")
    return udc


def _check_config(config: Section) -> None:
    if (
        not config.otg.acm.enabled
        and config.kvmd.hid.type != "otg"
        and config.kvmd.msd.type != "otg"
    ):
        raise RuntimeError("Nothing to do")


def _cmd_start(config: Section) -> None:
    # https://www.kernel.org/doc/Documentation/usb/gadget_configfs.txt
    # https://www.isticktoit.net/?p=1383

    _check_config(config)

    udc = _find_udc(config.otg.udc)

    gadget_path = join("/sys/kernel/config/usb_gadget", config.otg.gadget)
    mkdir(gadget_path)

    _write(join(gadget_path, "idVendor"), f"0x{config.otg.vendor_id:X}")
    _write(join(gadget_path, "idProduct"), f"0x{config.otg.product_id:X}")
    _write(join(gadget_path, "bcdDevice"), "0x0100")
    _write(join(gadget_path, "bcdUSB"), "0x0200")

    lang_path = join(gadget_path, "strings/0x409")
    mkdir(lang_path)
    _write(join(lang_path, "manufacturer"), config.otg.manufacturer)
    _write(join(lang_path, "product"), config.otg.product)
    _write(join(lang_path, "serialnumber"), config.otg.serial_number)

    config_path = join(gadget_path, "configs/c.1")
    mkdir(config_path)
    mkdir(join(config_path, "strings/0x409"))
    _write(join(config_path, "strings/0x409/configuration"), "Config 1: ECM network")
    _write(join(config_path, "MaxPower"), "250")

    if config.otg.acm.enabled:
        func_path = join(gadget_path, "functions/acm.usb0")
        mkdir(func_path)
        symlink(func_path, join(config_path, "acm.usb0"))

    if config.kvmd.hid.type == "otg":
        func_path = join(gadget_path, "functions/hid.usb0")
        mkdir(func_path)
        _write(join(func_path, "protocol"), "1")
        _write(join(func_path, "subclass"), "1")
        _write(join(func_path, "report_length"), "1")
        with open(join(func_path, "report_desc"), "wb") as report_file:
            report_file.write(
                b"\x05\x01\x09\x06\xa1\x01\x05\x07\x19\xe0\x29\xe7\x15\x00"
                b"\x25\x01\x75\x01\x95\x08\x81\x02\x95\x01\x75\x08\x81\x03"
                b"\x95\x05\x75\x01\x05\x08\x19\x01\x29\x05\x91\x02\x95\x01"
                b"\x75\x03\x91\x03\x95\x06\x75\x08\x15\x00\x25\x65\x05\x07"
                b"\x19\x00\x29\x65\x81\x00\xc0"
            )
        symlink(func_path, join(config_path, "hid.usb0"))

    if config.kvmd.msd.type == "otg":
        func_path = join(gadget_path, "functions/mass_storage.usb0")
        mkdir(func_path)
        _write(join(func_path, "stall"), "0")
        _write(join(func_path, "lun.0/cdrom"), "1")
        _write(join(func_path, "lun.0/ro"), "1")
        _write(join(func_path, "lun.0/removable"), "1")
        _write(join(func_path, "lun.0/nofua"), "0")
        symlink(func_path, join(config_path, "mass_storage.usb0"))

    _write(join(gadget_path, "UDC"), udc)

    time.sleep(config.otg.init_delay)


def _cmd_stop(config: Section) -> None:
    # https://www.kernel.org/doc/Documentation/usb/gadget_configfs.txt

    _check_config(config)

    gadget_path = join("/sys/kernel/config/usb_gadget", config.otg.gadget)

    _write(join(gadget_path, "UDC"), "")

    config_path = join(gadget_path, "configs/c.1")
    for func in listdir(config_path):
        if func.endswith(".usb0"):
            unlink(join(config_path, func))
    rmdir(join(config_path, "strings/0x409"))
    rmdir(config_path)

    funcs_path = join(gadget_path, "functions")
    for func in listdir(funcs_path):
        if func.endswith(".usb0"):
            rmdir(join(funcs_path, func))

    rmdir(join(gadget_path, "strings/0x409"))
    rmdir(gadget_path)


# =====
def main(argv: Optional[List[str]]=None) -> None:
    (parent_parser, argv, config) = init(
        add_help=False,
        argv=argv,
        load_hid=True,
        load_atx=True,
        load_msd=True,
    )
    parser = argparse.ArgumentParser(
        prog="kvmd-otg",
        description="Control KVMD OTG device",
        parents=[parent_parser],
    )
    parser.set_defaults(cmd=(lambda *_: parser.print_help()))
    subparsers = parser.add_subparsers()

    cmd_start_parser = subparsers.add_parser("start", help="Start OTG")
    cmd_start_parser.set_defaults(cmd=_cmd_start)

    cmd_stop_parser = subparsers.add_parser("stop", help="Stop OTG")
    cmd_stop_parser.set_defaults(cmd=_cmd_stop)

    options = parser.parse_args(argv[1:])
    try:
        options.cmd(config)
    except ValidatorError as err:
        raise SystemExit(str(err))
