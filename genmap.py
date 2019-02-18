#!/usr/bin/env python3
# ========================================================================== #
#                                                                            #
#    KVMD - The The main Pi-KVM daemon.                                      #
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


import textwrap

from typing import List
from typing import NamedTuple

import yaml


# =====
class KeyMapping(NamedTuple):
    kvmd_code: int
    arduino_hid_key: str
    js_key: str


# =====
def main() -> None:
    keymap: List[KeyMapping] = []
    with open("keymap.in") as keymap_file:
        for row in keymap_file:
            if not row.startswith("#"):
                parts = row.split()
                keymap.append(KeyMapping(
                    kvmd_code=int(parts[0]),
                    arduino_hid_key=parts[1],
                    js_key=parts[2],
                ))

    path = "kvmd/data/keymap.yaml"
    with open(path, "w") as keymap_yaml_file:
        keymap_yaml_file.write(textwrap.dedent("""
            # ========================================================================== #
            #                                                                            #
            #    KVMD - The The main Pi-KVM daemon.                                      #
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
        """).strip() + "\n\n\n")
        yaml.dump({
            km.js_key: km.kvmd_code
            for km in keymap
        }, keymap_yaml_file, indent=4, default_flow_style=False)
        print("Generated:", path)

    path = "hid/src/keymap.h"
    with open(path, "w") as hid_header_file:
        hid_header_file.write(textwrap.dedent("""
            /*****************************************************************************
            #                                                                            #
            #    KVMD - The The main Pi-KVM daemon.                                      #
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
            *****************************************************************************/
        """).strip() + "\n\n\n")
        hid_header_file.write("#pragma once\n\n#include <HID-Project.h>\n\n#include \"inline.h\"\n\n\n")
        hid_header_file.write("INLINE KeyboardKeycode keymap(uint8_t code) {\n\tswitch(code) {\n")
        for km in sorted(keymap, key=(lambda km: km.arduino_hid_key)):
            hid_header_file.write("\t\tcase {km.kvmd_code}: return {km.arduino_hid_key};\n".format(km=km))
        hid_header_file.write("\t\tdefault: return KEY_ERROR_UNDEFINED;\n\t}\n}\n")
        print("Generated:", path)


if __name__ == "__main__":
    main()
