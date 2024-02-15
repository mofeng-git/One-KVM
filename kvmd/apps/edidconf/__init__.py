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


import sys
import os
import re
import dataclasses
import contextlib
import subprocess
import argparse
import time

from typing import IO
from typing import Generator
from typing import Callable

from ...validators.basic import valid_bool
from ...validators.basic import valid_int_f0

# from .. import init


# =====
class NoBlockError(Exception):
    pass


@contextlib.contextmanager
def _smart_open(path: str, mode: str) -> Generator[IO, None, None]:
    fd = (0 if "r" in mode else 1)
    with (os.fdopen(fd, mode, closefd=False) if path == "-" else open(path, mode)) as file:
        yield file
        if "w" in mode:
            file.flush()


@dataclasses.dataclass(frozen=True)
class _CeaBlock:
    tag:  int
    data: bytes

    def __post_init__(self) -> None:
        assert 0 < self.tag <= 0b111
        assert 0 < len(self.data) <= 0b11111

    @property
    def size(self) -> int:
        return len(self.data) + 1

    def pack(self) -> bytes:
        header = (self.tag << 5) | len(self.data)
        return header.to_bytes() + self.data

    @classmethod
    def first_from_raw(cls, raw: (bytes | list[int])) -> "_CeaBlock":
        assert 0 < raw[0] <= 0xFF
        tag = (raw[0] & 0b11100000) >> 5
        data_size = (raw[0] & 0b00011111)
        data = bytes(raw[1:data_size + 1])
        return _CeaBlock(tag, data)


_CEA = 128
_CEA_AUDIO = 1
_CEA_SPEAKERS = 4


class _Edid:
    # https://en.wikipedia.org/wiki/Extended_Display_Identification_Data

    def __init__(self, path: str) -> None:
        with _smart_open(path, "rb") as file:
            data = file.read()
            if data.startswith(b"\x00\xFF\xFF\xFF\xFF\xFF\xFF\x00"):
                self.__data = list(data)
            else:
                text = re.sub(r"\s", "", data.decode())
                self.__data = [
                    int(text[index:index + 2], 16)
                    for index in range(0, len(text), 2)
                ]
            assert len(self.__data) == 256, f"Invalid EDID length: {len(self.__data)}, should be 256 bytes"
            assert self.__data[126] == 1, "Zero extensions number"
            assert (self.__data[_CEA + 0], self.__data[_CEA + 1]) == (0x02, 0x03), "Can't find CEA extension"

    def write_hex(self, path: str) -> None:
        self.__update_checksums()
        text = "\n".join(
            "".join(
                f"{item:0{2}X}"
                for item in self.__data[index:index + 16]
            )
            for index in range(0, len(self.__data), 16)
        ) + "\n"
        with _smart_open(path, "w") as file:
            file.write(text)

    def write_bin(self, path: str) -> None:
        self.__update_checksums()
        with _smart_open(path, "wb") as file:
            file.write(bytes(self.__data))

    def __update_checksums(self) -> None:
        self.__data[127] = 256 - (sum(self.__data[:127]) % 256)
        self.__data[255] = 256 - (sum(self.__data[128:255]) % 256)

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
        for ch in data:
            assert 0x41 <= ch <= 0x5A, "Mfc ID must contain only A-Z characters"
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
        return self.__get_dtd_text(0xFC, "Monitor Name")

    def set_monitor_name(self, text: str) -> None:
        self.__set_dtd_text(0xFC, "Monitor Name", text)

    def get_monitor_serial(self) -> str:
        return self.__get_dtd_text(0xFF, "Monitor Serial")

    def set_monitor_serial(self, text: str) -> None:
        self.__set_dtd_text(0xFF, "Monitor Serial", text)

    def __get_dtd_text(self, d_type: int, name: str) -> str:
        index = self.__find_dtd_text(d_type, name)
        return bytes(self.__data[index:index + 13]).decode("cp437").strip()

    def __set_dtd_text(self, d_type: int, name: str, text: str) -> None:
        index = self.__find_dtd_text(d_type, name)
        encoded = (text[:13] + "\n" + " " * 12)[:13].encode("cp437")
        for (offset, ch) in enumerate(encoded):
            self.__data[index + offset] = ch

    def __find_dtd_text(self, d_type: int, name: str) -> int:
        for index in [54, 72, 90, 108]:
            if self.__data[index + 3] == d_type:
                return index + 5
        raise NoBlockError(f"Can't find DTD {name}")

    # ===== CEA =====

    def get_audio(self) -> bool:
        (cbs, _) = self.__parse_cea()
        audio = False
        speakers = False
        for cb in cbs:
            if cb.tag == _CEA_AUDIO:
                audio = True
            elif cb.tag == _CEA_SPEAKERS:
                speakers = True
        return (audio and speakers and self.__get_basic_audio())

    def set_audio(self, enabled: bool) -> None:
        (cbs, dtds) = self.__parse_cea()
        cbs = [cb for cb in cbs if cb.tag not in [_CEA_AUDIO, _CEA_SPEAKERS]]
        if enabled:
            cbs.append(_CeaBlock(_CEA_AUDIO, b"\x09\x7f\x07"))
            cbs.append(_CeaBlock(_CEA_SPEAKERS, b"\x01\x00\x00"))
        self.__replace_cea(cbs, dtds)
        self.__set_basic_audio(enabled)

    def __get_basic_audio(self) -> bool:
        return bool(self.__data[_CEA + 3] & 0b01000000)

    def __set_basic_audio(self, enabled: bool) -> None:
        if enabled:
            self.__data[_CEA + 3] |= 0b01000000
        else:
            self.__data[_CEA + 3] &= (0xFF - 0b01000000)  # ~X

    def __parse_cea(self) -> tuple[list[_CeaBlock], bytes]:
        cea = self.__data[_CEA:]
        dtd_begin = cea[2]
        if dtd_begin == 0:
            return ([], b"")

        cbs: list[_CeaBlock] = []
        if dtd_begin > 4:
            raw = cea[4:dtd_begin]
            while len(raw) != 0:
                cb = _CeaBlock.first_from_raw(raw)
                cbs.append(cb)
                raw = raw[cb.size:]

        dtds = b""
        assert dtd_begin >= 4
        raw = cea[dtd_begin:]
        while len(raw) > (18 + 1) and raw[0] != 0:
            dtds += bytes(raw[:18])
            raw = raw[18:]

        return (cbs, dtds)

    def __replace_cea(self, cbs: list[_CeaBlock], dtds: bytes) -> None:
        cbs_packed = b""
        for cb in cbs:
            cbs_packed += cb.pack()

        raw = cbs_packed + dtds
        assert len(raw) <= (128 - 4 - 1), "Too many CEA blocks or DTDs"

        self.__data[_CEA + 2] = (0 if len(raw) == 0 else (len(cbs_packed) + 4))

        for index in range(4, 127):
            try:
                ch = raw[index - 4]
            except IndexError:
                ch = 0
            self.__data[_CEA + index] = ch


def _format_bool(value: bool) -> str:
    return ("yes" if value else "no")


def _make_format_hex(size: int) -> Callable[[int], str]:
    return (lambda value: ("0x{:0%dX} ({})" % (size * 2)).format(value, value))


# =====
def main(argv: (list[str] | None)=None) -> None:  # pylint: disable=too-many-branches,too-many-statements
    # (parent_parser, argv, _) = init(
    #     add_help=False,
    #     argv=argv,
    # )
    if argv is None:
        argv = sys.argv
    parser = argparse.ArgumentParser(
        prog="kvmd-edidconf",
        description="A simple and primitive KVMD EDID editor",
        # parents=[parent_parser],
    )
    parser.add_argument("-f", "--edid", dest="edid_path", default="/etc/kvmd/tc358743-edid.hex",
                        help="The hex/bin EDID file path", metavar="<file>")
    parser.add_argument("--export-hex",
                        help="Export [--edid] file to the new file as a hex text", metavar="<file>")
    parser.add_argument("--export-bin",
                        help="Export [--edid] file to the new file as a bin data", metavar="<file>")
    parser.add_argument("--import", dest="imp",
                        help="Import the specified bin/hex EDID to the [--edid] file as a hex text", metavar="<file>")
    parser.add_argument("--restore-default", choices=["v0", "v1", "v2", "v3", "v4mini", "v4plus"],
                        help="Restore default edid for the given PiKVM build")
    parser.add_argument("--set-audio", type=valid_bool,
                        help="Enable or disable audio", metavar="<yes|no>")
    parser.add_argument("--set-mfc-id",
                        help="Set manufacturer ID (https://uefi.org/pnp_id_list)", metavar="<ABC>")
    parser.add_argument("--set-product-id", type=valid_int_f0,
                        help="Set product ID (decimal)", metavar="<uint>")
    parser.add_argument("--set-serial", type=valid_int_f0,
                        help="Set serial number (decimal)", metavar="<uint>")
    parser.add_argument("--set-monitor-name",
                        help="Set monitor name in DTD block (ASCII, max 13 characters)", metavar="<str>")
    parser.add_argument("--set-monitor-serial",
                        help="Set monitor serial in DTD block if exists (ASCII, max 13 characters)", metavar="<str>")
    parser.add_argument("--clear", action="store_true",
                        help="Clear the EDID in the [--device]")
    parser.add_argument("--apply", action="store_true",
                        help="Apply [--edid] on the [--device]")
    parser.add_argument("--device", dest="device_path", default="/dev/kvmd-video",
                        help="The video device", metavar="<device>")
    options = parser.parse_args(argv[1:])

    if options.restore_default:
        options.imp = f"/usr/share/kvmd/configs.default/kvmd/edid/{options.restore_default}-hdmi.hex"

    orig_edid_path = options.edid_path
    if options.imp:
        options.export_hex = options.edid_path
        options.edid_path = options.imp

    edid = _Edid(options.edid_path)
    changed = False

    for cmd in dir(_Edid):
        if cmd.startswith("set_"):
            value = getattr(options, cmd)
            if value is not None:
                getattr(edid, cmd)(value)
                changed = True

    if options.export_hex is not None:
        edid.write_hex(options.export_hex)
    elif options.export_bin is not None:
        edid.write_bin(options.export_bin)
    elif changed:
        edid.write_hex(options.edid_path)

    for (key, get, fmt) in [
        ("Manufacturer ID:", edid.get_mfc_id,         str),
        ("Product ID:     ", edid.get_product_id,     _make_format_hex(2)),
        ("Serial number:  ", edid.get_serial,         _make_format_hex(4)),
        ("Monitor name:   ", edid.get_monitor_name,   str),
        ("Monitor serial: ", edid.get_monitor_serial, str),
        ("Audio:          ", edid.get_audio,          _format_bool),
    ]:
        try:
            print(key, fmt(get()), file=sys.stderr)  # type: ignore
        except NoBlockError:
            pass

    try:
        if options.clear:
            subprocess.run([
                "/usr/bin/v4l2-ctl",
                f"--device={options.device_path}",
                "--clear-edid",
            ], stdout=sys.stderr, check=True)
            if options.apply:
                time.sleep(1)
        if options.apply:
            subprocess.run([
                "/usr/bin/v4l2-ctl",
                f"--device={options.device_path}",
                f"--set-edid=file={orig_edid_path}",
                "--fix-edid-checksums",
                "--info-edid",
            ], stdout=sys.stderr, check=True)
    except subprocess.CalledProcessError as err:
        raise SystemExit(str(err))
