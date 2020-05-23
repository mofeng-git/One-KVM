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
import textwrap
import dataclasses

from typing import Set
from typing import List

import Xlib.keysymdef.latin1
import Xlib.keysymdef.miscellany

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
    code = getattr(Xlib.keysymdef.latin1, name, None)
    if code is None:
        code = getattr(Xlib.keysymdef.miscellany, name, None)
    assert code is not None, name
    return code


def _read_keymap_in(path: str) -> List[_KeyMapping]:
    keymap: List[_KeyMapping] = []
    with open(path) as keymap_file:
        for line in keymap_file:
            line = line.strip()
            if len(line) > 0 and not line.startswith("#"):
                parts = list(map(str.strip, line.split()))
                if len(parts) >= 6:
                    otg_is_modifier = parts[3].startswith("^")
                    otg_code = int((parts[3][1:] if otg_is_modifier else parts[3]), 16)

                    x11_keys: Set[_X11Key] = set()
                    for x11_name in parts[5].split(","):
                        x11_shift = x11_name.startswith("^")
                        x11_name = (x11_name[1:] if x11_shift else x11_name)
                        x11_code = _resolve_keysym(x11_name)
                        x11_keys.add(_X11Key(x11_name, x11_code, x11_shift))

                    keymap.append(_KeyMapping(
                        web_name=parts[0],
                        serial_code=int(parts[1]),
                        arduino_name=parts[2],
                        otg_key=_OtgKey(otg_code, otg_is_modifier),
                        at1_code=int(parts[4], 16),
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
    assert len(sys.argv) == 4, f"{sys.argv[0]} <keymap.in> <template> <out>"

    keymap = _read_keymap_in(sys.argv[1])
    _render_keymap(keymap, sys.argv[2], sys.argv[3])


# =====
if __name__ == "__main__":
    main()
