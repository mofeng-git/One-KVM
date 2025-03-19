# ========================================================================== #
#                                                                            #
#    KVMD - The main PiKVM daemon.                                           #
#                                                                            #
#    Copyright (C) 2018-2024  Maxim Devaev <mdevaev@gmail.com>               #
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
import re
import dataclasses
import contextlib

from typing import IO
from typing import Generator


# =====
class EdidNoBlockError(Exception):
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


_LONG = 256
_SHORT = 128

_CEA = 128
_CEA_AUDIO = 1
_CEA_SPEAKERS = 4


class Edid:
    # https://en.wikipedia.org/wiki/Extended_Display_Identification_Data

    def __init__(self, data: bytes) -> None:
        assert len(data) in [_SHORT, _LONG], f"Invalid EDID length: {len(data)}, should be {_SHORT} or {_LONG} bytes"
        self.__long = (len(data) == _LONG)
        if self.__long:
            assert data[126] == 1, "Zero extensions number"
            assert (data[_CEA + 0], data[_CEA + 1]) == (0x02, 0x03), "Can't find CEA extension"
        self.__data = list(data)

    @classmethod
    def is_header_valid(cls, data: bytes) -> bool:
        return data.startswith(b"\x00\xFF\xFF\xFF\xFF\xFF\xFF\x00")

    @classmethod
    def from_file(cls, path: str) -> "Edid":
        with _smart_open(path, "rb") as file:
            data = file.read()
            if not cls.is_header_valid(data):
                text = re.sub(r"\s", "", data.decode())
                data = bytes([
                    int(text[index:index + 2], 16)
                    for index in range(0, len(text), 2)
                ])
        return Edid(data)

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
        if self.__long:
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
        raise EdidNoBlockError(f"Can't find DTD {name}")

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
        if not self.__long:
            raise EdidNoBlockError("This EDID does not contain any CEA blocks")

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
