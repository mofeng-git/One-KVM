# ========================================================================== #
#                                                                            #
#    KVMD - The main PiKVM daemon.                                           #
#                                                                            #
#    Copyright (C) 2020  Maxim Devaev <mdevaev@gmail.com>                    #
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


import dataclasses

from typing import Any


# =====
class RfbEncodings:
    RESIZE = -223  # DesktopSize Pseudo-encoding
    RENAME = -307  # DesktopName Pseudo-encoding
    LEDS_STATE = -261  # QEMU LED State Pseudo-encoding
    EXT_KEYS = -258  # QEMU Extended Key Events Pseudo-encoding
    CONT_UPDATES = -313  # ContinuousUpdates Pseudo-encoding

    TIGHT = 7
    TIGHT_JPEG_QUALITIES = dict(zip(  # JPEG Quality Level Pseudo-encoding
        [-32, -31, -30, -29, -28, -27, -26, -25, -24, -23],
        [10,   20,  30,  40,  50,  60,  70,  80,  90, 100],
    ))

    H264 = 50  # Open H.264 Encoding


def _make_meta(variants: (int | frozenset[int])) -> dict:
    return {"variants": (frozenset([variants]) if isinstance(variants, int) else variants)}


@dataclasses.dataclass(frozen=True)
class RfbClientEncodings:  # pylint: disable=too-many-instance-attributes
    encodings: frozenset[int]

    has_resize: bool =		    dataclasses.field(default=False, metadata=_make_meta(RfbEncodings.RESIZE))  # noqa: E224
    has_rename: bool =		    dataclasses.field(default=False, metadata=_make_meta(RfbEncodings.RENAME))  # noqa: E224
    has_leds_state: bool =	    dataclasses.field(default=False, metadata=_make_meta(RfbEncodings.LEDS_STATE))  # noqa: E224
    has_ext_keys: bool =	    dataclasses.field(default=False, metadata=_make_meta(RfbEncodings.EXT_KEYS))  # noqa: E224
    has_cont_updates: bool =	dataclasses.field(default=False, metadata=_make_meta(RfbEncodings.CONT_UPDATES))  # noqa: E224

    has_tight: bool =		    dataclasses.field(default=False, metadata=_make_meta(RfbEncodings.TIGHT))  # noqa: E224
    tight_jpeg_quality: int =	dataclasses.field(default=0,     metadata=_make_meta(frozenset(RfbEncodings.TIGHT_JPEG_QUALITIES)))  # noqa: E224

    has_h264: bool =			dataclasses.field(default=False, metadata=_make_meta(RfbEncodings.H264))  # noqa: E224

    def get_summary(self) -> list[str]:
        summary: list[str] = [f"encodings -- {sorted(self.encodings)}"]
        for field in dataclasses.fields(self):
            if field.name != "encodings":
                found = ", ".join(map(str, sorted(map(int, self.__get_found(field)))))
                summary.append(f"{field.name} [{found}] -- {getattr(self, field.name)}")
        return summary

    def __post_init__(self) -> None:
        for field in dataclasses.fields(self):
            if field.name != "encodings":
                self.__set_value(field.name, bool(self.__get_found(field)))
        self.__set_value("tight_jpeg_quality", self.__get_tight_jpeg_quality())

    def __set_value(self, key: str, value: Any) -> None:
        object.__setattr__(self, key, value)

    def __get_found(self, field: dataclasses.Field) -> frozenset[int]:
        return self.encodings.intersection(field.metadata["variants"])

    def __get_tight_jpeg_quality(self) -> int:
        if self.has_tight:
            qualities = self.encodings.intersection(RfbEncodings.TIGHT_JPEG_QUALITIES)
            if qualities:
                return RfbEncodings.TIGHT_JPEG_QUALITIES[max(qualities)]
        return 0
