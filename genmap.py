#!/usr/bin/env python3
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


import sys
import csv
import textwrap
import dataclasses

from typing import Set
from typing import List
from typing import Optional

import Xlib.keysymdef.latin1
import Xlib.keysymdef.miscellany
import Xlib.keysymdef.xf86

import mako.template


# =====
@dataclasses.dataclass(frozen=True)
class _OtgKey:
    code: int
    is_modifier: bool


@dataclasses.dataclass(frozen=True)
class _X11Key:
    name: str
    code: int
    shift: bool


@dataclasses.dataclass(frozen=True)
class _KeyMapping:
    web_name: str
    serial_code: int
    arduino_name: str
    otg_key: _OtgKey
    at1_code: int
    x11_keys: Set[_X11Key]


def _resolve_keysym(name: str) -> int:
    code: Optional[int] = None
    for module in [
        Xlib.keysymdef.latin1,
        Xlib.keysymdef.miscellany,
        Xlib.keysymdef.xf86,
    ]:
        code = getattr(module, name, None)
        if code is not None:
            break
    assert code is not None, name
    return code


def _read_keymap_csv(path: str) -> List[_KeyMapping]:
    keymap: List[_KeyMapping] = []
    with open(path) as keymap_file:
        for row in csv.DictReader(keymap_file):
            if len(row) >= 6:
                otg_is_modifier = row["otg_key"].startswith("^")
                otg_code = int((row["otg_key"][1:] if otg_is_modifier else row["otg_key"]), 16)

                x11_keys: Set[_X11Key] = set()
                for x11_name in row["x11_names"].split(","):
                    x11_shift = x11_name.startswith("^")
                    x11_name = (x11_name[1:] if x11_shift else x11_name)
                    x11_code = _resolve_keysym(x11_name)
                    x11_keys.add(_X11Key(x11_name, x11_code, x11_shift))

                keymap.append(_KeyMapping(
                    web_name=row["web_name"],
                    serial_code=int(row["serial_code"]),
                    arduino_name=row["arduino_name"],
                    otg_key=_OtgKey(otg_code, otg_is_modifier),
                    at1_code=int(row["at1_code"], 16),
                    x11_keys=x11_keys,
                ))
    return keymap


def _render_keymap(keymap: List[_KeyMapping], template_path: str, out_path: str) -> None:
    with open(template_path) as template_file:
        with open(out_path, "w") as out_file:
            template = textwrap.dedent(template_file.read())
            rendered = mako.template.Template(template).render(keymap=keymap)
            out_file.write(rendered)


# =====
def main() -> None:
    # https://developer.mozilla.org/en-US/docs/Web/API/KeyboardEvent/code/code_values
    # https://github.com/NicoHood/HID/blob/master/src/KeyboardLayouts/ImprovedKeylayouts.h
    # https://gist.github.com/MightyPork/6da26e382a7ad91b5496ee55fdc73db2
    # https://github.com/qemu/keycodemapdb/blob/master/data/keymaps.csv

    # Fields list:
    #   - Web
    #   - Serial code
    #   - Arduino key
    #   - OTG code (^ for mod)
    #   - AT set1
    #   -X11 keysyms (^ for shift)

    assert len(sys.argv) == 4, f"{sys.argv[0]} <keymap.csv> <template> <out>"

    keymap = _read_keymap_csv(sys.argv[1])
    _render_keymap(keymap, sys.argv[2], sys.argv[3])


# =====
if __name__ == "__main__":
    main()
