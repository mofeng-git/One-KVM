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


import sys
import re
import argparse

from typing import List
from typing import Optional

from ...validators.basic import valid_bool

from .. import init


# =====
class _Edid:
    def __init__(self, path: str) -> None:
        with open(path) as file:
            self.__load_from_hex(file.read())

    def write(self, path: str) -> None:
        self.__update_checksums()
        text = "\n".join(
            "".join(
                f"{item:0{2}X}"
                for item in self.__data[index:index + 16]
            )
            for index in range(0, len(self.__data), 16)
        )
        if path:
            with open(path, "w") as file:
                file.write(text + "\n")
        else:
            print(text)

    def is_audio_enabled(self) -> bool:
        return bool(self.__data[131] & 0b01000000)

    def set_audio_enabled(self, enabled: bool) -> None:
        if enabled:
            self.__data[131] |= 0b01000000
        else:
            self.__data[131] &= (0xFF - 0b01000000)  # ~X

    def __load_from_hex(self, text: str) -> None:
        text = re.sub(r"\s", "", text)
        self.__data = [
            int(text[index:index + 2], 16)
            for index in range(0, len(text), 2)
        ]
        assert len(self.__data) == 256, f"Invalid EDID length: {len(self.__data)}, should be 256 bytes"
        assert self.__data[126] == 1, "Zero extensions number"
        assert (self.__data[128], self.__data[129]) == (0x02, 0x03), "Can't find CEA-861"

    def __update_checksums(self) -> None:
        self.__data[127] = 256 - (sum(self.__data[:127]) % 256)
        self.__data[255] = 256 - (sum(self.__data[128:255]) % 256)


# =====
def main(argv: Optional[List[str]]=None) -> None:
    (parent_parser, argv, _) = init(
        add_help=False,
        argv=argv,
    )
    parser = argparse.ArgumentParser(
        prog="kvmd-edidconf",
        description="A simple and primitive KVMD EDID editor",
        parents=[parent_parser],
    )
    parser.add_argument("-f", "--edid-file", dest="path", default="/etc/kvmd/tc358743-edid.hex",
                        help="EDID hex text file path", metavar="<file>")
    parser.add_argument("--stdout", action="store_true",
                        help="Write to stdout instead of the rewriting the source file")
    parser.add_argument("--set-audio", type=valid_bool, dest="set_audio", default=None,
                        help="Enable or disable basic audio", metavar="<yes|no>")
    parser.add_argument("--show-info", action="store_true",
                        help="Write summary info to stderr")
    options = parser.parse_args(argv[1:])

    edid = _Edid(options.path)
    changed = False

    if options.set_audio is not None:
        edid.set_audio_enabled(options.set_audio)
        changed = True

    if changed:
        edid.write("" if options.stdout else options.path)

    if options.show_info:
        print(f"Audio: {'yes' if edid.is_audio_enabled() else 'no'}", file=sys.stderr)
