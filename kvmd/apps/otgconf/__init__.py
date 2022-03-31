# ========================================================================== #
#                                                                            #
#    KVMD - The main PiKVM daemon.                                           #
#                                                                            #
#    Copyright (C) 2018-2022  Maxim Devaev <mdevaev@gmail.com>               #
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
import json
import contextlib
import argparse

from typing import List
from typing import Generator
from typing import Optional

from ...validators.basic import valid_stripped_string_not_empty

from ... import env
from ... import usb

from .. import init


# =====
def _make_config_path(gadget: str, func: str) -> str:
    return os.path.join(f"{env.SYSFS_PREFIX}/sys/kernel/config/usb_gadget", gadget, "configs/c.1", func)


def _make_func_path(gadget: str, func: str) -> str:
    return os.path.join(f"{env.SYSFS_PREFIX}/sys/kernel/config/usb_gadget", gadget, "functions", func)


@contextlib.contextmanager
def _udc_stopped(gadget: str, udc: str) -> Generator[None, None, None]:
    udc = usb.find_udc(udc)
    udc_path = os.path.join(f"{env.SYSFS_PREFIX}/sys/kernel/config/usb_gadget", gadget, "UDC")
    with open(udc_path) as udc_file:
        enabled = bool(udc_file.read().strip())
    if enabled:
        with open(udc_path, "w") as udc_file:
            udc_file.write("\n")
    try:
        yield
    finally:
        if enabled:
            with open(udc_path, "w") as udc_file:
                udc_file.write(udc)


def _enable_function(gadget: str, udc: str, func: str) -> None:
    with _udc_stopped(gadget, udc):
        os.symlink(_make_func_path(gadget, func), _make_config_path(gadget, func))


def _disable_function(gadget: str, udc: str, func: str) -> None:
    with _udc_stopped(gadget, udc):
        os.unlink(_make_config_path(gadget, func))


def _list_functions(gadget: str, meta_path: str) -> None:
    for meta_name in sorted(os.listdir(meta_path)):
        with open(os.path.join(meta_path, meta_name)) as meta_file:
            meta = json.loads(meta_file.read())
        enabled = os.path.exists(_make_config_path(gadget, meta["func"]))
        print(f"{'+' if enabled else '-'} {meta['func']}  # {meta['name']}")


def _reset_gadget(gadget: str, udc: str) -> None:
    with _udc_stopped(gadget, udc):
        pass


# =====
def main(argv: Optional[List[str]]=None) -> None:
    (parent_parser, argv, config) = init(
        add_help=False,
        argv=argv,
    )
    parser = argparse.ArgumentParser(
        prog="kvmd-otgconf",
        description="KVMD OTG low-level runtime configuration tool",
        parents=[parent_parser],
    )
    parser.add_argument("-l", "--list-functions", action="store_true", help="List functions")
    parser.add_argument("-e", "--enable-function", type=valid_stripped_string_not_empty,
                        metavar="<name>", help="Enable function")
    parser.add_argument("-d", "--disable-function", type=valid_stripped_string_not_empty,
                        metavar="<name>", help="Disable function")
    parser.add_argument("-r", "--reset-gadget", action="store_true", help="Reset gadget")
    options = parser.parse_args(argv[1:])

    if options.reset_gadget:
        _reset_gadget(config.otg.gadget, config.otg.udc)
        return
    elif options.enable_function:
        _enable_function(config.otg.gadget, config.otg.udc, options.enable_function)
    elif options.disable_function:
        _disable_function(config.otg.gadget, config.otg.udc, options.disable_function)
    _list_functions(config.otg.gadget, config.otg.meta)
