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
from ...validators.basic import valid_int_f0

from .. import init


# =====
class _Edid:
    # https://en.wikipedia.org/wiki/Extended_Display_Identification_Data

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

    # =====

    def get_mfc_id(self) -> str:
        raw = self.__data[8] << 8 | self.__data[9]
        return bytes([
            ((raw >> 10) & 0b11111) + 0x40,
            ((raw >> 5) & 0b11111) + 0x40,
            (raw & 0b11111) + 0x40,
        ]).decode("ascii")

    def set_mfc_id(self, mfc_id: str) -> None:
        assert len(mfc_id) == 3, "Mfc ID must be 3 characters long"
        data = mfc_id.upper().encode("ascii")
        for byte in data:
            assert 0x41 <= byte <= 0x5A, "Mfc ID must contain only A-Z characters"
        raw = (
            (data[2] - 0x40)
            | ((data[1] - 0x40) << 5)
            | ((data[0] - 0x40) << 10)
        )
        self.__data[8] = (raw >> 8) & 0xFF
        self.__data[9] = raw & 0xFF

    # =====

    def get_product_id(self) -> int:
        return (self.__data[10] | self.__data[11] << 8)

    def set_product_id(self, product_id: int) -> None:
        assert 0 <= product_id <= 0xFFFF, f"Product ID should be from 0 to {0xFFFF}"
        self.__data[10] = product_id & 0xFF
        self.__data[11] = (product_id >> 8) & 0xFF

    # =====

    def get_serial(self) -> int:
        return (
            self.__data[12]
            | self.__data[13] << 8
            | self.__data[14] << 16
            | self.__data[15] << 24
        )

    def set_serial(self, serial: int) -> None:
        assert 0 <= serial <= 0xFFFFFFFF, f"Serial should be from 0 to {0xFFFFFFFF}"
        self.__data[12] = serial & 0xFF
        self.__data[13] = (serial >> 8) & 0xFF
        self.__data[14] = (serial >> 16) & 0xFF
        self.__data[15] = (serial >> 24) & 0xFF

    # =====

    def get_monitor_name(self) -> str:
        index = self.__find_dtd_value(0xFC)
        assert index > 0, "Can't find DTD Monitor name"
        return bytes(self.__data[index:index + 13]).decode("cp437").strip()

    def set_monitor_name(self, name: str) -> None:
        index = self.__find_dtd_value(0xFC)
        assert index > 0, "Can't find DTD Monitor name"
        encoded = (name[:13] + "\n" + " " * 12)[:13].encode("cp437")
        for (offset, byte) in enumerate(encoded):
            self.__data[index + offset] = byte

    # =====

    def get_audio(self) -> bool:
        return bool(self.__data[131] & 0b01000000)

    def set_audio(self, enabled: bool) -> None:
        if enabled:
            self.__data[131] |= 0b01000000
        else:
            self.__data[131] &= (0xFF - 0b01000000)  # ~X

    # =====

    def __find_dtd_value(self, dtype: int) -> int:
        for index in [54, 72, 90, 108]:
            if self.__data[index + 3] == dtype:
                return index + 5
        return -1

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
    parser.add_argument("--set-audio", type=valid_bool, default=None,
                        help="Enable or disable basic audio", metavar="<yes|no>")
    parser.add_argument("--set-mfc-id", default=None,
                        help="Set manufacturer ID (https://uefi.org/pnp_id_list)", metavar="<ABC>")
    parser.add_argument("--set-product-id", type=valid_int_f0, default=None,
                        help="Set product ID (decimal)", metavar="<uint>")
    parser.add_argument("--set-serial", type=valid_int_f0, default=None,
                        help="Set serial number (decimal)", metavar="<uint>")
    parser.add_argument("--set-monitor-name", default=None,
                        help="Set monitor name in DTD/MND (ASCII, max 13 characters)", metavar="<str>")
    parser.add_argument("--show-info", action="store_true",
                        help="Write summary info to stderr")
    options = parser.parse_args(argv[1:])

    edid = _Edid(options.path)
    changed = False

    for cmd in dir(options):
        if cmd.startswith("set_"):
            value = getattr(options, cmd)
            if value is not None:
                getattr(edid, cmd)(value)
                changed = True

    if changed:
        edid.write("" if options.stdout else options.path)

    if options.show_info:
        print("Mfc ID:      ", edid.get_mfc_id())
        print("Product ID:  ", edid.get_product_id())
        print("Serial:      ", edid.get_serial())
        print("Monitor name:", edid.get_monitor_name())
        print("Audio:       ", ("yes" if edid.get_audio() else "no"), file=sys.stderr)
