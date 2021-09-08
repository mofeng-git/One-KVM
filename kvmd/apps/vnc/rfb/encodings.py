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

from typing import FrozenSet
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


@dataclasses.dataclass(frozen=True)
class RfbClientEncodings:  # pylint: disable=too-many-instance-attributes
    encodings: FrozenSet[int]

    has_resize: bool = dataclasses.field(default=False)
    has_rename: bool = dataclasses.field(default=False)
    has_leds_state: bool = dataclasses.field(default=False)
    has_ext_keys: bool = dataclasses.field(default=False)

    has_tight: bool = dataclasses.field(default=False)
    tight_jpeg_quality: int = dataclasses.field(default=0)

    has_h264: bool = dataclasses.field(default=False)

    def __post_init__(self) -> None:
        self.__set("has_resize", (RfbEncodings.RESIZE in self.encodings))
        self.__set("has_rename", (RfbEncodings.RENAME in self.encodings))
        self.__set("has_leds_state", (RfbEncodings.LEDS_STATE in self.encodings))
        self.__set("has_ext_keys", (RfbEncodings.EXT_KEYS in self.encodings))

        self.__set("has_tight", (RfbEncodings.TIGHT in self.encodings))
        self.__set("tight_jpeg_quality", self.__get_tight_jpeg_quality())

        self.__set("has_h264", (RfbEncodings.H264 in self.encodings))

    def __set(self, key: str, value: Any) -> None:
        object.__setattr__(self, key, value)

    def __get_tight_jpeg_quality(self) -> int:
        if RfbEncodings.TIGHT in self.encodings:
            qualities = self.encodings.intersection(RfbEncodings.TIGHT_JPEG_QUALITIES)
            if qualities:
                return RfbEncodings.TIGHT_JPEG_QUALITIES[max(qualities)]
        return 0
