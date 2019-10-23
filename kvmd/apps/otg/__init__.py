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
import re
import shutil
import time
import argparse

from os.path import join  # pylint: disable=ungrouped-imports

from typing import List
from typing import Optional

from ...logging import get_logger

from ...yamlconf import Section

from ...validators import ValidatorError

from .. import init

from .hid import Hid
from .hid.keyboard import KEYBOARD_HID
from .hid.mouse import MOUSE_HID


# =====
def _mkdir(path: str) -> None:
    get_logger().info("MKDIR --- %s", path)
    os.mkdir(path)


def _chown(path: str, user: str) -> None:
    get_logger().info("CHOWN --- %s - %s", user, path)
    shutil.chown(path, user)


def _symlink(src: str, dest: str) -> None:
    get_logger().info("SYMLINK - %s --> %s", dest, src)
    os.symlink(src, dest)


def _rmdir(path: str) -> None:
    get_logger().info("RMDIR --- %s", path)
    os.rmdir(path)


def _unlink(path: str) -> None:
    get_logger().info("RM ------ %s", path)
    os.unlink(path)


def _write(path: str, text: str) -> None:
    get_logger().info("WRITE --- %s", path)
    with open(path, "w") as param_file:
        param_file.write(text)


def _write_bytes(path: str, data: bytes) -> None:
    get_logger().info("WRITE --- %s", path)
    with open(path, "wb") as param_file:
        param_file.write(data)


def _find_udc(udc: str) -> str:
    udcs = sorted(os.listdir("/sys/class/udc"))
    if not udc:
        if len(udcs) == 0:
            raise RuntimeError("Can't find any UDC")
        udc = udcs[0]
    elif udc not in udcs:
        raise RuntimeError(f"Can't find selected UDC: {udc}")
    get_logger().info("Using UDC %s", udc)
    return udc


def _check_config(config: Section) -> None:
    if (
        not config.otg.acm.enabled
        and config.kvmd.hid.type != "otg"
        and config.kvmd.msd.type != "otg"
    ):
        raise RuntimeError("Nothing to do")


# =====
def _create_acm(gadget_path: str, config_path: str) -> None:
    func_path = join(gadget_path, "functions/acm.usb0")
    _mkdir(func_path)
    _symlink(func_path, join(config_path, "acm.usb0"))


def _create_hid(gadget_path: str, config_path: str, instance: int, hid: Hid) -> None:
    func_path = join(gadget_path, f"functions/hid.usb{instance}")
    _mkdir(func_path)
    _write(join(func_path, "protocol"), str(hid.protocol))
    _write(join(func_path, "subclass"), str(hid.subclass))
    _write(join(func_path, "report_length"), str(hid.report_length))
    _write_bytes(join(func_path, "report_desc"), hid.report_descriptor)
    _symlink(func_path, join(config_path, f"hid.usb{instance}"))


def _create_msd(
    gadget_path: str,
    config_path: str,
    instance: int,
    user: str,
    stall: bool,
    cdrom: bool,
    rw: bool,
    removable: bool,
    fua: bool,
) -> None:

    func_path = join(gadget_path, f"functions/mass_storage.usb{instance}")
    _mkdir(func_path)
    _write(join(func_path, "stall"), str(int(stall)))
    _write(join(func_path, "lun.0/cdrom"), str(int(cdrom)))
    _write(join(func_path, "lun.0/ro"), str(int(not rw)))
    _write(join(func_path, "lun.0/removable"), str(int(removable)))
    _write(join(func_path, "lun.0/nofua"), str(int(not fua)))
    if user != "root":
        _chown(join(func_path, "lun.0/cdrom"), user)
        _chown(join(func_path, "lun.0/ro"), user)
        _chown(join(func_path, "lun.0/file"), user)
    _symlink(func_path, join(config_path, f"mass_storage.usb{instance}"))


def _cmd_start(config: Section) -> None:
    # https://www.kernel.org/doc/Documentation/usb/gadget_configfs.txt
    # https://www.isticktoit.net/?p=1383

    logger = get_logger()

    _check_config(config)

    udc = _find_udc(config.otg.udc)

    logger.info("Creating gadget %r ...", config.otg.gadget)
    gadget_path = join("/sys/kernel/config/usb_gadget", config.otg.gadget)
    _mkdir(gadget_path)

    _write(join(gadget_path, "idVendor"), f"0x{config.otg.vendor_id:X}")
    _write(join(gadget_path, "idProduct"), f"0x{config.otg.product_id:X}")
    _write(join(gadget_path, "bcdDevice"), "0x0100")
    _write(join(gadget_path, "bcdUSB"), "0x0200")

    lang_path = join(gadget_path, "strings/0x409")
    _mkdir(lang_path)
    _write(join(lang_path, "manufacturer"), config.otg.manufacturer)
    _write(join(lang_path, "product"), config.otg.product)
    _write(join(lang_path, "serialnumber"), config.otg.serial)

    config_path = join(gadget_path, "configs/c.1")
    _mkdir(config_path)
    _mkdir(join(config_path, "strings/0x409"))
    _write(join(config_path, "strings/0x409/configuration"), "Config 1: ECM network")
    _write(join(config_path, "MaxPower"), "250")

    if config.otg.acm.enabled:
        logger.info("Required ACM")
        _create_acm(gadget_path, config_path)

    if config.kvmd.hid.type == "otg":
        logger.info("Required HID")
        _create_hid(gadget_path, config_path, 0, KEYBOARD_HID)
        _create_hid(gadget_path, config_path, 1, MOUSE_HID)

    if config.kvmd.msd.type == "otg":
        logger.info("Required MSD")
        _create_msd(gadget_path, config_path, 0, config.otg.msd.user, **config.otg.msd.default._unpack())  # pylint: disable=protected-access
        if config.otg.drives.enabled:
            logger.info("Required MSD extra drives: %d", config.otg.drives.count)
            for instance in range(config.otg.drives.count):
                _create_msd(gadget_path, config_path, instance + 1, "root", **config.otg.drives.default._unpack())  # pylint: disable=protected-access

    logger.info("Enabling the gadget ...")
    _write(join(gadget_path, "UDC"), udc)
    time.sleep(config.otg.init_delay)

    logger.info("Ready to work")


# =====
def _cmd_stop(config: Section) -> None:
    # https://www.kernel.org/doc/Documentation/usb/gadget_configfs.txt

    logger = get_logger()

    _check_config(config)

    gadget_path = join("/sys/kernel/config/usb_gadget", config.otg.gadget)

    logger.info("Disabling gadget %r ...", config.otg.gadget)
    _write(join(gadget_path, "UDC"), "")

    config_path = join(gadget_path, "configs/c.1")
    for func in os.listdir(config_path):
        if re.search(r"\.usb\d+$", func):
            _unlink(join(config_path, func))
    _rmdir(join(config_path, "strings/0x409"))
    _rmdir(config_path)

    funcs_path = join(gadget_path, "functions")
    for func in os.listdir(funcs_path):
        if re.search(r"\.usb\d+$", func):
            _rmdir(join(funcs_path, func))

    _rmdir(join(gadget_path, "strings/0x409"))
    _rmdir(gadget_path)

    logger.info("Bye-bye")


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
