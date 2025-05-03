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


import struct
import dataclasses

from typing import Optional

from .types import Edid
from .types import Colors


# =====
class Packable:
    def pack(self) -> bytes:
        raise NotImplementedError()


class Unpackable:
    @classmethod
    def unpack(cls, data: bytes, offset: int=0) -> "Unpackable":
        raise NotImplementedError()


# =====
@dataclasses.dataclass(frozen=True)
class Header(Packable, Unpackable):
    proto:   int
    rid:     int
    op:      int
    unit:    int

    NAK        = 0
    BOOTLOADER = 2
    REBOOT     = 3
    STATE      = 4
    SWITCH     = 5
    BEACON     = 6
    ATX_LEDS   = 7
    ATX_CLICK  = 8
    SET_EDID   = 9
    CLEAR_EDID = 10
    SET_COLORS = 12
    SET_QUIRKS = 13
    SET_DUMMY  = 14

    __struct = struct.Struct("<BHBB")

    SIZE = __struct.size

    def pack(self) -> bytes:
        return self.__struct.pack(self.proto, self.rid, self.op, self.unit)

    @classmethod
    def unpack(cls, data: bytes, offset: int=0) -> "Header":
        return Header(*cls.__struct.unpack_from(data, offset=offset))


@dataclasses.dataclass(frozen=True)
class Nak(Unpackable):
    reason: int

    INVALID_COMMAND   = 0
    BUSY              = 1
    NO_DOWNLINK       = 2
    DOWNLINK_OVERFLOW = 3

    __struct = struct.Struct("<B")

    @classmethod
    def unpack(cls, data: bytes, offset: int=0) -> "Nak":
        return Nak(*cls.__struct.unpack_from(data, offset=offset))


@dataclasses.dataclass(frozen=True)
class UnitVersion:
    hw:     int
    sw:     int
    sw_dev: bool

    def is_fresh(self, version: int) -> bool:
        return (self.sw_dev or (self.sw >= version))


@dataclasses.dataclass(frozen=True)
class UnitFlags:
    changing_busy: bool
    flashing_busy: bool
    has_downlink:  bool
    has_hpd:       bool


@dataclasses.dataclass(frozen=True)
class UnitQuirks:
    ignore_hpd: bool


@dataclasses.dataclass(frozen=True)
class UnitState(Unpackable):  # pylint: disable=too-many-instance-attributes
    version:       UnitVersion
    flags:         UnitFlags
    ch:            int
    beacons:       tuple[bool, bool, bool, bool, bool, bool]
    np_crc:        tuple[int,  int,  int,  int,  int,  int]
    video_5v_sens: tuple[bool, bool, bool, bool, bool]
    video_hpd:     tuple[bool, bool, bool, bool, bool]
    video_edid:    tuple[bool, bool, bool, bool]
    video_crc:     tuple[int,  int,  int,  int]
    video_dummies: tuple[bool, bool, bool, bool]
    usb_5v_sens:   tuple[bool, bool, bool, bool]
    atx_busy:      tuple[bool, bool, bool, bool]
    quirks:        UnitQuirks

    __struct = struct.Struct("<HHHBBHHHHHHBBBHHHHBxBBB28x")

    def compare_edid(self, ch: int, edid: Optional["Edid"]) -> bool:
        if edid is None:
            # Сойдет любой невалидный EDID
            return (not self.video_edid[ch])
        return (
            self.video_edid[ch] == edid.valid
            and self.video_crc[ch] == edid.crc
        )

    @classmethod
    def unpack(cls, data: bytes, offset: int=0) -> "UnitState":  # pylint: disable=too-many-locals
        (
            sw_version, hw_version, flags, ch,
            beacons, nc0, nc1, nc2, nc3, nc4, nc5,
            video_5v_sens, video_hpd, video_edid, vc0, vc1, vc2, vc3,
            usb_5v_sens, atx_busy, quirks, video_dummies,
        ) = cls.__struct.unpack_from(data, offset=offset)
        return UnitState(
            version=UnitVersion(
                hw=hw_version,
                sw=(sw_version & 0x7FFF),
                sw_dev=bool(sw_version & 0x8000),
            ),
            flags=UnitFlags(
                changing_busy=bool(flags & 0x80),
                flashing_busy=bool(flags & 0x40),
                has_downlink=bool(flags & 0x02),
                has_hpd=bool(flags & 0x04),
            ),
            ch=ch,
            beacons=cls.__make_flags6(beacons),
            np_crc=(nc0, nc1, nc2, nc3, nc4, nc5),
            video_5v_sens=cls.__make_flags5(video_5v_sens),
            video_hpd=cls.__make_flags5(video_hpd),
            video_edid=cls.__make_flags4(video_edid),
            video_crc=(vc0, vc1, vc2, vc3),
            video_dummies=cls.__make_flags4(video_dummies),
            usb_5v_sens=cls.__make_flags4(usb_5v_sens),
            atx_busy=cls.__make_flags4(atx_busy),
            quirks=UnitQuirks(ignore_hpd=bool(quirks & 0x01)),
        )

    @classmethod
    def __make_flags6(cls, mask: int) -> tuple[bool, bool, bool, bool, bool, bool]:
        return (
            bool(mask & 0x01), bool(mask & 0x02), bool(mask & 0x04),
            bool(mask & 0x08), bool(mask & 0x10), bool(mask & 0x20),
        )

    @classmethod
    def __make_flags5(cls, mask: int) -> tuple[bool, bool, bool, bool, bool]:
        return (
            bool(mask & 0x01), bool(mask & 0x02), bool(mask & 0x04),
            bool(mask & 0x08), bool(mask & 0x10),
        )

    @classmethod
    def __make_flags4(cls, mask: int) -> tuple[bool, bool, bool, bool]:
        return (bool(mask & 0x01), bool(mask & 0x02), bool(mask & 0x04), bool(mask & 0x08))


@dataclasses.dataclass(frozen=True)
class UnitAtxLeds(Unpackable):
    power: tuple[bool, bool, bool, bool]
    hdd:   tuple[bool, bool, bool, bool]

    __struct = struct.Struct("<B")

    @classmethod
    def unpack(cls, data: bytes, offset: int=0) -> "UnitAtxLeds":
        (mask,) = cls.__struct.unpack_from(data, offset=offset)
        return UnitAtxLeds(
            power=(bool(mask & 0x01), bool(mask & 0x02), bool(mask & 0x04), bool(mask & 0x08)),
            hdd=(bool(mask & 0x10), bool(mask & 0x20), bool(mask & 0x40), bool(mask & 0x80)),
        )


# =====
@dataclasses.dataclass(frozen=True)
class BodySwitch(Packable):
    ch: int

    def __post_init__(self) -> None:
        assert 0 <= self.ch <= 4

    def pack(self) -> bytes:
        return self.ch.to_bytes()


@dataclasses.dataclass(frozen=True)
class BodySetBeacon(Packable):
    ch: int
    on: bool

    def __post_init__(self) -> None:
        assert 0 <= self.ch <= 5

    def pack(self) -> bytes:
        return self.ch.to_bytes() + self.on.to_bytes()


@dataclasses.dataclass(frozen=True)
class BodyAtxClick(Packable):
    ch:       int
    action:   int
    delay_ms: int

    POWER = 0
    RESET = 1

    __struct = struct.Struct("<BBH")

    def __post_init__(self) -> None:
        assert 0 <= self.ch <= 3
        assert self.action in [self.POWER, self.RESET]
        assert 1 <= self.delay_ms <= 0xFFFF

    def pack(self) -> bytes:
        return self.__struct.pack(self.ch, self.action, self.delay_ms)


@dataclasses.dataclass(frozen=True)
class BodySetEdid(Packable):
    ch:   int
    edid: Edid

    def __post_init__(self) -> None:
        assert 0 <= self.ch <= 3

    def pack(self) -> bytes:
        return self.ch.to_bytes() + self.edid.pack()


@dataclasses.dataclass(frozen=True)
class BodyClearEdid(Packable):
    ch: int

    def __post_init__(self) -> None:
        assert 0 <= self.ch <= 3

    def pack(self) -> bytes:
        return self.ch.to_bytes()


@dataclasses.dataclass(frozen=True)
class BodySetDummy(Packable):
    ch: int
    on: bool

    def __post_init__(self) -> None:
        assert 0 <= self.ch <= 3

    def pack(self) -> bytes:
        return self.ch.to_bytes() + self.on.to_bytes()


@dataclasses.dataclass(frozen=True)
class BodySetColors(Packable):
    ch:     int
    colors: Colors

    def __post_init__(self) -> None:
        assert 0 <= self.ch <= 5

    def pack(self) -> bytes:
        return self.ch.to_bytes() + self.colors.pack()


@dataclasses.dataclass(frozen=True)
class BodySetQuirks(Packable):
    ignore_hpd: bool

    def pack(self) -> bytes:
        return self.ignore_hpd.to_bytes()


# =====
@dataclasses.dataclass(frozen=True)
class Request:
    header: Header
    body:   (Packable | None) = dataclasses.field(default=None)

    def pack(self) -> bytes:
        msg = self.header.pack()
        if self.body is not None:
            msg += self.body.pack()
        return msg


@dataclasses.dataclass(frozen=True)
class Response:
    header: Header
    body:   Unpackable

    @classmethod
    def unpack(cls, msg: bytes) -> Optional["Response"]:
        header = Header.unpack(msg)
        match header.op:
            case Header.NAK:
                return Response(header, Nak.unpack(msg, Header.SIZE))
            case Header.STATE:
                return Response(header, UnitState.unpack(msg, Header.SIZE))
            case Header.ATX_LEDS:
                return Response(header, UnitAtxLeds.unpack(msg, Header.SIZE))
        # raise RuntimeError(f"Unknown OP in the header: {header!r}")
        return None
