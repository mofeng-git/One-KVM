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

from typing import List
from typing import FrozenSet
from typing import Union
from typing import Any


# =====
class RfbEncodings:
    RESIZE = -223  # DesktopSize Pseudo-encoding
    RENAME = -307  # DesktopName Pseudo-encoding
    LEDS_STATE = -261  # QEMU LED State Pseudo-encoding
    EXT_KEYS = -258  # QEMU Extended Key Events Pseudo-encoding

    TIGHT = 7
    TIGHT_JPEG_QUALITIES = dict(zip(  # JPEG Quality Level Pseudo-encoding
        [-32, -31, -30, -29, -28, -27, -26, -25, -24, -23],
        [10,   20,  30,  40,  50,  60,  70,  80,  90, 100],
    ))

    H264 = 50  # Open H.264 Encoding


def _feature(default: Any, variants: Union[int, FrozenSet[int]]) -> dataclasses.Field:
    return dataclasses.field(default=default, metadata={
        "variants": (frozenset([variants]) if isinstance(variants, int) else variants),
    })


@dataclasses.dataclass(frozen=True)
class RfbClientEncodings:  # pylint: disable=too-many-instance-attributes
    encodings: FrozenSet[int]

    has_resize: bool =		    _feature(False, RfbEncodings.RESIZE)
    has_rename: bool =		    _feature(False, RfbEncodings.RENAME)
    has_leds_state: bool =	    _feature(False, RfbEncodings.LEDS_STATE)
    has_ext_keys: bool =	    _feature(False, RfbEncodings.EXT_KEYS)

    has_tight: bool =		    _feature(False, RfbEncodings.TIGHT)
    tight_jpeg_quality: int =	_feature(0,     frozenset(RfbEncodings.TIGHT_JPEG_QUALITIES))

    has_h264: bool =			_feature(False, RfbEncodings.H264)

    def get_summary(self) -> List[str]:
        summary: List[str] = [f"encodings -- {sorted(self.encodings)}"]
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

    def __get_found(self, field: dataclasses.Field) -> None:
        return self.encodings.intersection(field.metadata["variants"])

    def __get_tight_jpeg_quality(self) -> int:
        if self.has_tight:
            qualities = self.encodings.intersection(RfbEncodings.TIGHT_JPEG_QUALITIES)
            if qualities:
                return RfbEncodings.TIGHT_JPEG_QUALITIES[max(qualities)]
        return 0
