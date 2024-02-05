# ========================================================================== #
#                                                                            #
#    KVMD - The main PiKVM daemon.                                           #
#                                                                            #
#    Copyright (C) 2018-2023  Maxim Devaev <mdevaev@gmail.com>               #
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
            self.__recreate_profile()
            time.sleep(self.__init_delay)
            with open(udc_path, "w") as file:
                file.write(udc)

    def __recreate_profile(self) -> None:
        # XXX: See pikvm/pikvm#1235
        # After unbind and bind, the gadgets stop working,
        # unless we recreate their links in the profile.
        # Some kind of kernel bug.
        for func in os.listdir(self.__get_fdest_path()):
            path = self.__get_fdest_path(func)
            if os.path.islink(path):
                try:
                    os.unlink(path)
                    os.symlink(self.__get_fsrc_path(func), path)
                except (FileNotFoundError, FileExistsError):
                    pass

    def __read_metas(self) -> Generator[dict, None, None]:
        for meta_name in sorted(os.listdir(self.__meta_path)):
            with open(os.path.join(self.__meta_path, meta_name)) as file:
                yield json.loads(file.read())

    def __get_fsrc_path(self, func: str) -> str:
        return usb.get_gadget_path(self.__gadget, usb.G_FUNCTIONS, func)

    def __get_fdest_path(self, func: (str | None)=None) -> str:
        if func is None:
            return usb.get_gadget_path(self.__gadget, usb.G_PROFILE)
        return usb.get_gadget_path(self.__gadget, usb.G_PROFILE, func)

    def enable_functions(self, funcs: list[str]) -> None:
        with self.__udc_stopped():
            for func in funcs:
                os.symlink(self.__get_fsrc_path(func), self.__get_fdest_path(func))

    def disable_functions(self, funcs: list[str]) -> None:
        with self.__udc_stopped():
            for func in funcs:
                os.unlink(self.__get_fdest_path(func))

    def list_functions(self) -> None:
        for meta in self.__read_metas():
            enabled = os.path.exists(self.__get_fdest_path(meta["func"]))
            print(f"{'+' if enabled else '-'} {meta['func']}  # {meta['name']}")

    def make_gpio_config(self) -> None:
        class Dumper(yaml.Dumper):
            def increase_indent(self, flow: bool=False, indentless: bool=False) -> None:
                _ = indentless
                super().increase_indent(flow, False)

            def ignore_aliases(self, data) -> bool:  # type: ignore
                _ = data
                return True

        class InlineList(list):
            pass

        def represent_inline_list(dumper: yaml.Dumper, data):  # type: ignore
            return dumper.represent_sequence("tag:yaml.org,2002:seq", data, flow_style=True)

        Dumper.add_representer(InlineList, represent_inline_list)

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
                "pulse": False,
            }
            config["view"]["table"].append(InlineList([  # type: ignore
                "#" + meta["name"],
                "#" + meta["func"],
                meta["func"],
            ]))
        print(yaml.dump({"kvmd": {"gpio": config}}, indent=4, Dumper=Dumper))

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
    parser.add_argument("-e", "--enable-function", nargs="+", metavar="<name>", help="Enable function(s)")
    parser.add_argument("-d", "--disable-function", nargs="+", metavar="<name>", help="Disable function(s)")
    parser.add_argument("-r", "--reset-gadget", action="store_true", help="Reset gadget")
    parser.add_argument("--make-gpio-config", action="store_true")
    options = parser.parse_args(argv[1:])

    gc = _GadgetControl(config.otg.meta, config.otg.gadget, config.otg.udc, config.otg.init_delay)

    if options.list_functions:
        gc.list_functions()

    elif options.enable_function:
        funcs = list(map(valid_stripped_string_not_empty, options.enable_function))
        gc.enable_functions(funcs)
        gc.list_functions()

    elif options.disable_function:
        funcs = list(map(valid_stripped_string_not_empty, options.disable_function))
        gc.disable_functions(funcs)
        gc.list_functions()

    elif options.reset_gadget:
        gc.reset()

    elif options.make_gpio_config:
        gc.make_gpio_config()

    else:
        gc.list_functions()
