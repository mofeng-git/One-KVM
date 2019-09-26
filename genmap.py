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

from typing import List

import mako.template


# =====
@dataclasses.dataclass(frozen=True)
class _KeyMapping:
    web_key: str
    serial_code: int
    arduino_key: str
    otg_code: int
    otg_is_modifier: bool


def _read_keymap_in(path: str) -> List[_KeyMapping]:
    keymap: List[_KeyMapping] = []
    with open(path) as keymap_file:
        for line in keymap_file:
            line = line.strip()
            if len(line) > 0 and not line.startswith("#"):
                parts = list(map(str.strip, line.split()))
                if len(parts) >= 5:
                    keymap.append(_KeyMapping(
                        web_key=parts[0],
                        serial_code=int(parts[1]),
                        arduino_key=parts[2],
                        otg_code=int(parts[3], 16),
                        otg_is_modifier=(parts[4].lower() == "m"),
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
