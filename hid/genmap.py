#!/usr/bin/env python3


import operator

from typing import Tuple
from typing import List

import yaml


# =====
def main() -> None:
    keymap: List[Tuple[int, str, str]] = []
    with open("keymap.in") as keymap_file:
        for row in keymap_file:
            if not row.startswith("#"):
                parts = row.split()
                keymap.append((int(parts[0]), parts[1], parts[2]))

    with open("../kvmd/kvmd/data/keymap.yaml", "w") as kvmd_yaml_file:
        yaml.dump({
            js_key: code
            for (code, _, js_key) in sorted(keymap, key=operator.itemgetter(2))
        }, kvmd_yaml_file, indent=4, default_flow_style=False)

    with open("src/keymap.h", "w") as hid_header_file:
        hid_header_file.write("#include <HID-Project.h>\n\n#include \"inline.h\"\n\n\n")
        hid_header_file.write("INLINE KeyboardKeycode keymap(uint8_t code) {\n\tswitch(code) {\n")
        for (code, hid_key, _) in sorted(keymap, key=operator.itemgetter(1)):
            hid_header_file.write("\t\tcase %d: return %s;\n" % (code, hid_key))
        hid_header_file.write("\t\tdefault: return 0;\n\t}\n}\n")


if __name__ == "__main__":
    main()
