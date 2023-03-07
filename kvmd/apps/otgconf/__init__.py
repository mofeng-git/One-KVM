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
import time

from typing import Generator

import yaml

from ...validators.basic import valid_stripped_string_not_empty

from ... import usb

from .. import init


# =====
class _GadgetControl:
    def __init__(self, meta_path: str, gadget: str, udc: str, init_delay: float) -> None:
        self.__meta_path = meta_path
        self.__gadget = gadget
        self.__udc = udc
        self.__init_delay = init_delay

    @contextlib.contextmanager
    def __udc_stopped(self) -> Generator[None, None, None]:
        udc = usb.find_udc(self.__udc)
        udc_path = usb.get_gadget_path(self.__gadget, usb.G_UDC)
        with open(udc_path) as file:
            enabled = bool(file.read().strip())
        if enabled:
            with open(udc_path, "w") as file:
                file.write("\n")
        try:
            yield
        finally:
            time.sleep(self.__init_delay)
            with open(udc_path, "w") as file:
                file.write(udc)

    def __read_metas(self) -> Generator[dict, None, None]:
        for meta_name in sorted(os.listdir(self.__meta_path)):
            with open(os.path.join(self.__meta_path, meta_name)) as file:
                yield json.loads(file.read())

    def enable_function(self, func: str) -> None:
        with self.__udc_stopped():
            os.symlink(
                usb.get_gadget_path(self.__gadget, usb.G_FUNCTIONS, func),
                usb.get_gadget_path(self.__gadget, usb.G_PROFILE, func),
            )

    def disable_function(self, func: str) -> None:
        with self.__udc_stopped():
            os.unlink(usb.get_gadget_path(self.__gadget, usb.G_PROFILE, func))

    def list_functions(self) -> None:
        for meta in self.__read_metas():
            enabled = os.path.exists(usb.get_gadget_path(self.__gadget, usb.G_PROFILE, meta["func"]))
            print(f"{'+' if enabled else '-'} {meta['func']}  # {meta['name']}")

    def make_gpio_config(self) -> None:
        config = {
            "drivers": {"otgconf": {"type": "otgconf"}},
            "scheme": {},
            "view": {"table": []},
        }
        for meta in self.__read_metas():
            config["scheme"][meta["func"]] = {  # type: ignore
                "driver": "otgconf",
                "pin": meta["func"],
                "mode": "output",
                "pulse": {"delay": 0},
            }
            config["view"]["table"].append([  # type: ignore
                "#" + meta["name"],
                "#" + meta["func"],
                meta["func"],
            ])
        print(yaml.dump({"kvmd": {"gpio": config}}, indent=4))

    def reset(self) -> None:
        with self.__udc_stopped():
            pass


# =====
def main(argv: (list[str] | None)=None) -> None:
    (parent_parser, argv, config) = init(
        add_help=False,
        cli_logging=True,
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
    parser.add_argument("--make-gpio-config", action="store_true")
    options = parser.parse_args(argv[1:])

    gc = _GadgetControl(config.otg.meta, config.otg.gadget, config.otg.udc, config.otg.init_delay)

    if options.list_functions:
        gc.list_functions()

    elif options.enable_function:
        gc.enable_function(options.enable_function)
        gc.list_functions()

    elif options.disable_function:
        gc.disable_function(options.disable_function)
        gc.list_functions()

    elif options.reset_gadget:
        gc.reset()

    elif options.make_gpio_config:
        gc.make_gpio_config()

    else:
        gc.list_functions()
