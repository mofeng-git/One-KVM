#!/usr/bin/env python3


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

    with open("kvmd/data/keymap.yaml", "w") as keymap_yaml_file:
        yaml.dump({
            km.js_key: km.kvmd_code
            for km in keymap
        }, keymap_yaml_file, indent=4, default_flow_style=False)

    with open("hid/src/keymap.h", "w") as hid_header_file:
        hid_header_file.write("#pragma once\n\n#include <HID-Project.h>\n\n#include \"inline.h\"\n\n\n")
        hid_header_file.write("INLINE KeyboardKeycode keymap(uint8_t code) {\n\tswitch(code) {\n")
        for km in sorted(keymap, key=(lambda km: km.arduino_hid_key)):
            hid_header_file.write("\t\tcase {km.kvmd_code}: return {km.arduino_hid_key};\n".format(km=km))
        hid_header_file.write("\t\tdefault: return KEY_ERROR_UNDEFINED;\n\t}\n}\n")


if __name__ == "__main__":
    main()
