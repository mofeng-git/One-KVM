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


import os
import re
import shutil
import json
import time
import argparse

from os.path import join  # pylint: disable=ungrouped-imports

from ...logging import get_logger

from ...yamlconf import Section

from ...validators import ValidatorError

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


def _unlink(path: str, optional: bool=False) -> None:
    logger = get_logger()
    if optional and not os.access(path, os.F_OK):
        logger.info("RM ------ [SKIPPED] %s", path)
        return
    logger.info("RM ------ %s", path)
    os.unlink(path)


def _write(path: str, value: (str | int), optional: bool=False) -> None:
    logger = get_logger()
    if optional and not os.access(path, os.F_OK):
        logger.info("WRITE --- [SKIPPED] %s", path)
        return
    logger.info("WRITE --- %s", path)
    with open(path, "w") as file:
        file.write(str(value))


def _write_bytes(path: str, data: bytes) -> None:
    get_logger().info("WRITE --- %s", path)
    with open(path, "wb") as file:
        file.write(data)


def _check_config(config: Section) -> None:
    if (
        not config.otg.devices.serial.enabled
        and not config.otg.devices.ethernet.enabled
        and config.kvmd.hid.type != "otg"
        and config.kvmd.msd.type != "otg"
    ):
        raise RuntimeError("Nothing to do")


# =====
class _GadgetConfig:
    def __init__(self, gadget_path: str, profile_path: str, meta_path: str, eps: int) -> None:
        self.__gadget_path = gadget_path
        self.__profile_path = profile_path
        self.__meta_path = meta_path
        self.__eps_max = eps
        self.__eps_used = 0
        self.__hid_instance = 0
        self.__msd_instance = 0
        _mkdir(meta_path)

    def add_serial(self, start: bool) -> None:
        eps = 3
        func = "acm.usb0"
        self.__create_function(func)
        if start:
            self.__start_function(func, eps)
        self.__create_meta(func, "Serial Port", eps)

    def add_ethernet(self, start: bool, driver: str, host_mac: str, kvm_mac: str) -> None:
        eps = 3
        if host_mac and kvm_mac and host_mac == kvm_mac:
            raise RuntimeError("Ethernet host_mac should not be equal to kvm_mac")
        real_driver = driver
        if driver == "rndis5":
            real_driver = "rndis"
        func = f"{real_driver}.usb0"
        func_path = self.__create_function(func)
        if host_mac:
            _write(join(func_path, "host_addr"), host_mac)
        if kvm_mac:
            _write(join(func_path, "dev_addr"), kvm_mac)
        if driver in ["ncm", "rndis"]:
            _write(join(self.__gadget_path, "os_desc/use"), "1")
            _write(join(self.__gadget_path, "os_desc/b_vendor_code"), "0xCD")
            _write(join(self.__gadget_path, "os_desc/qw_sign"), "MSFT100")
            if driver == "ncm":
                _write(join(func_path, "os_desc/interface.ncm/compatible_id"), "WINNCM")
            elif driver == "rndis":
                # On Windows 7 and later, the RNDIS 5.1 driver would be used by default,
                # but it does not work very well. The RNDIS 6.0 driver works better.
                # In order to get this driver to load automatically, we have to use
                # a Microsoft-specific extension of USB.
                _write(join(func_path, "os_desc/interface.rndis/compatible_id"), "RNDIS")
                _write(join(func_path, "os_desc/interface.rndis/sub_compatible_id"), "5162001")
            _symlink(self.__profile_path, join(self.__gadget_path, "os_desc", usb.G_PROFILE_NAME))
        if start:
            self.__start_function(func, eps)
        self.__create_meta(func, "Ethernet", eps)

    def add_keyboard(self, start: bool, remote_wakeup: bool) -> None:
        self.__add_hid("Keyboard", start, remote_wakeup, make_keyboard_hid())

    def add_mouse(self, start: bool, remote_wakeup: bool, absolute: bool, horizontal_wheel: bool) -> None:
        desc = ("Absolute" if absolute else "Relative") + " Mouse"
        self.__add_hid(desc, start, remote_wakeup, make_mouse_hid(absolute, horizontal_wheel))

    def __add_hid(self, desc: str, start: bool, remote_wakeup: bool, hid: Hid) -> None:
        eps = 1
        func = f"hid.usb{self.__hid_instance}"
        func_path = self.__create_function(func)
        _write(join(func_path, "no_out_endpoint"), "1", optional=True)
        if remote_wakeup:
            _write(join(func_path, "wakeup_on_write"), "1", optional=True)
        _write(join(func_path, "protocol"), hid.protocol)
        _write(join(func_path, "subclass"), hid.subclass)
        _write(join(func_path, "report_length"), hid.report_length)
        _write_bytes(join(func_path, "report_desc"), hid.report_descriptor)
        if start:
            self.__start_function(func, eps)
        self.__create_meta(func, desc, eps)
        self.__hid_instance += 1

    def add_msd(self, start: bool, user: str, stall: bool, cdrom: bool, rw: bool, removable: bool, fua: bool) -> None:
        # Endpoints number depends on transport_type but we can consider that this is 2
        # because transport_type is always USB_PR_BULK by default if CONFIG_USB_FILE_STORAGE_TEST
        # is not defined. See drivers/usb/gadget/function/storage_common.c
        eps = 2
        func = f"mass_storage.usb{self.__msd_instance}"
        func_path = self.__create_function(func)
        _write(join(func_path, "stall"), int(stall))
        _write(join(func_path, "lun.0/cdrom"), int(cdrom))
        _write(join(func_path, "lun.0/ro"), int(not rw))
        _write(join(func_path, "lun.0/removable"), int(removable))
        _write(join(func_path, "lun.0/nofua"), int(not fua))
        if user != "root":
            _chown(join(func_path, "lun.0/cdrom"), user)
            _chown(join(func_path, "lun.0/ro"), user)
            _chown(join(func_path, "lun.0/file"), user)
            _chown(join(func_path, "lun.0/forced_eject"), user)
        if start:
            self.__start_function(func, eps)
        desc = ("Mass Storage Drive" if self.__msd_instance == 0 else f"Extra Drive #{self.__msd_instance}")
        self.__create_meta(func, desc, eps)
        self.__msd_instance += 1

    def __create_function(self, func: str) -> str:
        func_path = join(self.__gadget_path, "functions", func)
        _mkdir(func_path)
        return func_path

    def __start_function(self, func: str, eps: int) -> None:
        func_path = join(self.__gadget_path, "functions", func)
        if self.__eps_max - self.__eps_used >= eps:
            _symlink(func_path, join(self.__profile_path, func))
            self.__eps_used += eps
        else:
            get_logger().info("Will not be started: No available endpoints")

    def __create_meta(self, func: str, desc: str, eps: int) -> None:
        _write(join(self.__meta_path, f"{func}@meta.json"), json.dumps({
            "function": func,
            "description": desc,
            "endpoints": eps,
        }))


def _cmd_start(config: Section) -> None:  # pylint: disable=too-many-statements,too-many-branches
    # https://www.kernel.org/doc/Documentation/usb/gadget_configfs.txt
    # https://www.isticktoit.net/?p=1383

    logger = get_logger()

    _check_config(config)

    udc = usb.find_udc(config.otg.udc)
    logger.info("Using UDC %s", udc)

    logger.info("Creating gadget %r ...", config.otg.gadget)
    gadget_path = usb.get_gadget_path(config.otg.gadget)
    _mkdir(gadget_path)

    _write(join(gadget_path, "idVendor"), f"0x{config.otg.vendor_id:04X}")
    _write(join(gadget_path, "idProduct"), f"0x{config.otg.product_id:04X}")
    _write(join(gadget_path, "bcdUSB"), f"0x{config.otg.usb_version:04X}")

    # bcdDevice should be incremented any time there are breaking changes
    # to this script so that the host OS sees it as a new device
    # and re-enumerates everything rather than relying on cached values.
    device_version = config.otg.device_version
    if device_version < 0:
        device_version = 0x0100
        if config.otg.devices.ethernet.enabled:
            if config.otg.devices.ethernet.driver == "ncm":
                device_version = 0x0102
            elif config.otg.devices.ethernet.driver == "rndis":
                device_version = 0x0101
    _write(join(gadget_path, "bcdDevice"), f"0x{device_version:04X}")

    lang_path = join(gadget_path, "strings/0x409")
    _mkdir(lang_path)
    _write(join(lang_path, "manufacturer"), config.otg.manufacturer)
    _write(join(lang_path, "product"), config.otg.product)
    if config.otg.serial is not None:
        _write(join(lang_path, "serialnumber"), config.otg.serial)

    profile_path = join(gadget_path, usb.G_PROFILE)
    _mkdir(profile_path)
    _mkdir(join(profile_path, "strings/0x409"))
    _write(join(profile_path, "strings/0x409/configuration"), f"Config 1: {config.otg.config}")
    _write(join(profile_path, "MaxPower"), config.otg.max_power)
    if config.otg.remote_wakeup:
        # XXX: Should we use MaxPower=100 with Remote Wakeup?
        _write(join(profile_path, "bmAttributes"), "0xA0")

    gc = _GadgetConfig(gadget_path, profile_path, config.otg.meta, config.otg.endpoints)
    cod = config.otg.devices

    if config.kvmd.hid.type == "otg":
        logger.info("===== HID-Keyboard =====")
        gc.add_keyboard(cod.hid.keyboard.start, config.otg.remote_wakeup)
        logger.info("===== HID-Mouse =====")
        ckhm = config.kvmd.hid.mouse
        gc.add_mouse(cod.hid.mouse.start, config.otg.remote_wakeup, ckhm.absolute, ckhm.horizontal_wheel)
        if config.kvmd.hid.mouse_alt.device:
            logger.info("===== HID-Mouse-Alt =====")
            gc.add_mouse(cod.hid.mouse_alt.start, config.otg.remote_wakeup, (not ckhm.absolute), ckhm.horizontal_wheel)

    if config.kvmd.msd.type == "otg":
        logger.info("===== MSD =====")
        gc.add_msd(cod.msd.start, config.otg.user, **cod.msd.default._unpack())
        if cod.drives.enabled:
            for count in range(cod.drives.count):
                logger.info("===== MSD Extra: %d =====", count + 1)
                gc.add_msd(cod.drives.start, "root", **cod.drives.default._unpack())

    if cod.ethernet.enabled:
        logger.info("===== Ethernet =====")
        gc.add_ethernet(**cod.ethernet._unpack(ignore=["enabled"]))

    if cod.serial.enabled:
        logger.info("===== Serial =====")
        gc.add_serial(cod.serial.start)

    logger.info("===== Preparing complete =====")

    logger.info("Enabling the gadget ...")
    _write(join(gadget_path, "UDC"), udc)
    time.sleep(config.otg.init_delay)
    _chown(join(gadget_path, "UDC"), config.otg.user)
    _chown(profile_path, config.otg.user)

    logger.info("Ready to work")


# =====
def _cmd_stop(config: Section) -> None:
    # https://www.kernel.org/doc/Documentation/usb/gadget_configfs.txt

    logger = get_logger()

    _check_config(config)

    gadget_path = usb.get_gadget_path(config.otg.gadget)

    logger.info("Disabling gadget %r ...", config.otg.gadget)
    _write(join(gadget_path, "UDC"), "\n")

    _unlink(join(gadget_path, "os_desc", usb.G_PROFILE_NAME), optional=True)

    profile_path = join(gadget_path, usb.G_PROFILE)
    for func in os.listdir(profile_path):
        if re.search(r"\.usb\d+$", func):
            _unlink(join(profile_path, func))
    _rmdir(join(profile_path, "strings/0x409"))
    _rmdir(profile_path)

    funcs_path = join(gadget_path, "functions")
    for func in os.listdir(funcs_path):
        if re.search(r"\.usb\d+$", func):
            _rmdir(join(funcs_path, func))

    _rmdir(join(gadget_path, "strings/0x409"))
    _rmdir(gadget_path)

    for meta in os.listdir(config.otg.meta):
        _unlink(join(config.otg.meta, meta))
    _rmdir(config.otg.meta)

    logger.info("Bye-bye")


# =====
def main(argv: (list[str] | None)=None) -> None:
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
    except ValidatorError as ex:
        raise SystemExit(str(ex))
