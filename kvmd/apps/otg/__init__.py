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

from ... import env
from ... import usb

from .. import init

from .hid import Hid
from .hid.keyboard import make_keyboard_hid
from .hid.mouse import make_mouse_hid


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


def _write(path: str, text: str, optional: bool=False) -> None:
    logger = get_logger()
    if optional and not os.access(path, os.F_OK):
        logger.info("SKIP ---- %s", path)
        return
    logger.info("WRITE --- %s", path)
    with open(path, "w") as param_file:
        param_file.write(text)


def _write_bytes(path: str, data: bytes) -> None:
    get_logger().info("WRITE --- %s", path)
    with open(path, "wb") as param_file:
        param_file.write(data)


def _check_config(config: Section) -> None:
    if (
        not config.otg.devices.serial.enabled
        and not config.otg.devices.ethernet.enabled
        and config.kvmd.hid.type != "otg"
        and config.kvmd.msd.type != "otg"
    ):
        raise RuntimeError("Nothing to do")


# =====
def _create_serial(gadget_path: str, config_path: str) -> None:
    func_path = join(gadget_path, "functions/acm.usb0")
    _mkdir(func_path)
    _symlink(func_path, join(config_path, "acm.usb0"))


def _create_ethernet(gadget_path: str, config_path: str, driver: str, host_mac: str, kvm_mac: str) -> None:
    if host_mac and kvm_mac and host_mac == kvm_mac:
        raise RuntimeError("Ethernet host_mac should not be equal to kvm_mac")
    func_path = join(gadget_path, f"functions/{driver}.usb0")
    _mkdir(func_path)
    if host_mac:
        _write(join(func_path, "host_addr"), host_mac)
    if kvm_mac:
        _write(join(func_path, "dev_addr"), kvm_mac)
    _symlink(func_path, join(config_path, f"{driver}.usb0"))


def _create_hid(gadget_path: str, config_path: str, instance: int, remote_wakeup: bool, hid: Hid) -> None:
    func_path = join(gadget_path, f"functions/hid.usb{instance}")
    _mkdir(func_path)
    _write(join(func_path, "no_out_endpoint"), "1", optional=True)
    if remote_wakeup:
        _write(join(func_path, "wakeup_on_write"), "1", optional=True)
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


def _cmd_start(config: Section) -> None:  # pylint: disable=too-many-statements
    # https://www.kernel.org/doc/Documentation/usb/gadget_configfs.txt
    # https://www.isticktoit.net/?p=1383

    logger = get_logger()

    _check_config(config)

    (udc, usb_driver) = usb.find_udc(config.otg.udc)
    logger.info("Using UDC %s", udc)

    logger.info("Creating gadget %r ...", config.otg.gadget)
    gadget_path = join(f"{env.SYSFS_PREFIX}/sys/kernel/config/usb_gadget", config.otg.gadget)
    _mkdir(gadget_path)

    _write(join(gadget_path, "idVendor"), f"0x{config.otg.vendor_id:04X}")
    _write(join(gadget_path, "idProduct"), f"0x{config.otg.product_id:04X}")
    _write(join(gadget_path, "bcdDevice"), "0x0100")
    _write(join(gadget_path, "bcdUSB"), f"0x{config.otg.usb_version:04X}")

    lang_path = join(gadget_path, "strings/0x409")
    _mkdir(lang_path)
    _write(join(lang_path, "manufacturer"), config.otg.manufacturer)
    _write(join(lang_path, "product"), config.otg.product)
    _write(join(lang_path, "serialnumber"), config.otg.serial)

    config_path = join(gadget_path, "configs/c.1")
    _mkdir(config_path)
    _mkdir(join(config_path, "strings/0x409"))
    _write(join(config_path, "strings/0x409/configuration"), f"Config 1: {config.otg.config}")
    _write(join(config_path, "MaxPower"), "250")
    if config.otg.remote_wakeup:
        # XXX: Should we use MaxPower=100 with Remote Wakeup?
        _write(join(config_path, "bmAttributes"), "0xA0")

    if config.otg.devices.serial.enabled:
        logger.info("===== Serial =====")
        _create_serial(gadget_path, config_path)

    if config.otg.devices.ethernet.enabled:
        logger.info("===== Ethernet =====")
        _create_ethernet(gadget_path, config_path, **config.otg.devices.ethernet._unpack(ignore=["enabled"]))

    if config.kvmd.hid.type == "otg":
        logger.info("===== HID-Keyboard =====")
        _create_hid(gadget_path, config_path, 0, config.otg.remote_wakeup, make_keyboard_hid())
        logger.info("===== HID-Mouse =====")
        _create_hid(gadget_path, config_path, 1, config.otg.remote_wakeup, make_mouse_hid(
            absolute=config.kvmd.hid.mouse.absolute,
            horizontal_wheel=config.kvmd.hid.mouse.horizontal_wheel,
        ))
        if config.kvmd.hid.mouse_alt.device:
            logger.info("===== HID-Mouse-Alt =====")
            _create_hid(gadget_path, config_path, 2, config.otg.remote_wakeup, make_mouse_hid(
                absolute=(not config.kvmd.hid.mouse.absolute),
                horizontal_wheel=config.kvmd.hid.mouse_alt.horizontal_wheel,
            ))

    if config.kvmd.msd.type == "otg":
        logger.info("===== MSD =====")
        _create_msd(gadget_path, config_path, 0, config.otg.user, **config.otg.devices.msd.default._unpack())
        if config.otg.devices.drives.enabled:
            for instance in range(config.otg.devices.drives.count):
                logger.info("===== MSD Extra: %d =====", config.otg.devices.drives.count)
                _create_msd(gadget_path, config_path, instance + 1, "root", **config.otg.devices.drives.default._unpack())

    logger.info("===== Preparing complete =====")

    logger.info("Enabling the gadget ...")
    _write(join(gadget_path, "UDC"), udc)
    time.sleep(config.otg.init_delay)

    logger.info("Setting %s bind permissions ...", usb_driver)
    driver_path = f"{env.SYSFS_PREFIX}/sys/bus/platform/drivers/{usb_driver}"
    _chown(join(driver_path, "bind"), config.otg.user)
    _chown(join(driver_path, "unbind"), config.otg.user)

    logger.info("Ready to work")


# =====
def _cmd_stop(config: Section) -> None:
    # https://www.kernel.org/doc/Documentation/usb/gadget_configfs.txt

    logger = get_logger()

    _check_config(config)

    gadget_path = join(f"{env.SYSFS_PREFIX}/sys/kernel/config/usb_gadget", config.otg.gadget)

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
