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


import re
import struct
import uuid
import dataclasses

from typing import TypeVar
from typing import Generic

from .lib import bitbang
from .lib import ParsedEdidNoBlockError
from .lib import ParsedEdid


# =====
@dataclasses.dataclass(frozen=True)
class EdidInfo:
    mfc_id:         str
    product_id:     int
    serial:         int
    monitor_name:   (str | None)
    monitor_serial: (str | None)
    audio:          bool

    @classmethod
    def from_data(cls, data: bytes) -> "EdidInfo":
        parsed = ParsedEdid(data)

        monitor_name: (str | None) = None
        try:
            monitor_name = parsed.get_monitor_name()
        except ParsedEdidNoBlockError:
            pass

        monitor_serial: (str | None) = None
        try:
            monitor_serial = parsed.get_monitor_serial()
        except ParsedEdidNoBlockError:
            pass

        audio: bool = False
        try:
            audio = parsed.get_audio()
        except ParsedEdidNoBlockError:
            pass

        return EdidInfo(
            mfc_id=parsed.get_mfc_id(),
            product_id=parsed.get_product_id(),
            serial=parsed.get_serial(),
            monitor_name=monitor_name,
            monitor_serial=monitor_serial,
            audio=audio,
        )


@dataclasses.dataclass(frozen=True)
class Edid:
    name:     str
    data:     bytes
    crc:      int = dataclasses.field(default=0)
    valid:    bool = dataclasses.field(default=False)
    info:     (EdidInfo | None) = dataclasses.field(default=None)
    _packed:  bytes = dataclasses.field(default=b"")

    def __post_init__(self) -> None:
        assert len(self.name) > 0
        assert len(self.data) in [128, 256]
        object.__setattr__(self, "_packed", (self.data + (b"\x00" * 128))[:256])
        object.__setattr__(self, "crc", bitbang.make_crc16(self._packed))  # Calculate CRC for filled data
        object.__setattr__(self, "valid", ParsedEdid.is_header_valid(self.data))
        try:
            object.__setattr__(self, "info", EdidInfo.from_data(self.data))
        except Exception:
            pass

    def as_text(self) -> str:
        return "".join(f"{item:0{2}X}" for item in self.data)

    def pack(self) -> bytes:
        return self._packed

    @classmethod
    def from_data(cls, name: str, data: (str | bytes | None)) -> "Edid":
        if data is None:  # Пустой едид
            return Edid(name, b"\x00" * 256)

        if isinstance(data, bytes):
            if ParsedEdid.is_header_valid(cls.data):
                return Edid(name, data)  # Бинарный едид
            data_hex = data.decode()  # Текстовый едид, прочитанный как бинарный из файла
        else:  # isinstance(data, str)
            data_hex = str(data)  # Текстовый едид

        data_hex = re.sub(r"\s", "", data_hex)
        assert len(data_hex) in [256, 512]
        data = bytes([
            int(data_hex[index:index + 2], 16)
            for index in range(0, len(data_hex), 2)
        ])
        return Edid(name, data)


@dataclasses.dataclass
class Edids:
    DEFAULT_NAME = "Default"
    DEFAULT_ID = "default"

    all:  dict[str, Edid] = dataclasses.field(default_factory=dict)
    port: dict[int, str] = dataclasses.field(default_factory=dict)

    def __post_init__(self) -> None:
        if self.DEFAULT_ID not in self.all:
            self.set_default(None)

    def set_default(self, data: (str | bytes | None)) -> None:
        self.all[self.DEFAULT_ID] = Edid.from_data(self.DEFAULT_NAME, data)

    def copy(self) -> "Edids":
        return Edids(dict(self.all), dict(self.port))

    def compare_on_ports(self, other: "Edids", ports: int) -> bool:
        for port in range(ports):
            if self.get_id_for_port(port) != other.get_id_for_port(port):
                return False
        return True

    def add(self, edid: Edid) -> str:
        edid_id = str(uuid.uuid4()).lower()
        self.all[edid_id] = edid
        return edid_id

    def set(self, edid_id: str, edid: Edid) -> None:
        assert edid_id in self.all
        self.all[edid_id] = edid

    def get(self, edid_id: str) -> Edid:
        return self.all[edid_id]

    def remove(self, edid_id: str) -> None:
        assert edid_id in self.all
        self.all.pop(edid_id)
        for port in list(self.port):
            if self.port[port] == edid_id:
                self.port.pop(port)

    def has_id(self, edid_id: str) -> bool:
        return (edid_id in self.all)

    def assign(self, port: int, edid_id: str) -> None:
        assert edid_id in self.all
        if edid_id == Edids.DEFAULT_ID:
            self.port.pop(port, None)
        else:
            self.port[port] = edid_id

    def get_id_for_port(self, port: int) -> str:
        return self.port.get(port, self.DEFAULT_ID)

    def get_edid_for_port(self, port: int) -> Edid:
        return self.all[self.get_id_for_port(port)]


# =====
@dataclasses.dataclass(frozen=True)
class Color:
    COMPONENTS = frozenset(["red", "green", "blue", "brightness", "blink_ms"])

    red:        int
    green:      int
    blue:       int
    brightness: int
    blink_ms:   int
    crc:        int = dataclasses.field(default=0)
    _packed:    bytes = dataclasses.field(default=b"")

    __struct = struct.Struct("<BBBBH")
    __rx = re.compile(r"^([0-9a-fA-F]{2})([0-9a-fA-F]{2})([0-9a-fA-F]{2}):([0-9a-fA-F]{2}):([0-9a-fA-F]{4})$")

    def __post_init__(self) -> None:
        assert 0 <= self.red <= 0xFF
        assert 0 <= self.green <= 0xFF
        assert 0 <= self.blue <= 0xFF
        assert 0 <= self.brightness <= 0xFF
        assert 0 <= self.blink_ms <= 0xFFFF
        data = self.__struct.pack(self.red, self.green, self.blue, self.brightness, self.blink_ms)
        object.__setattr__(self, "crc", bitbang.make_crc16(data))
        object.__setattr__(self, "_packed", data)

    def pack(self) -> bytes:
        return self._packed

    @classmethod
    def from_text(cls, text: str) -> "Color":
        match = cls.__rx.match(text)
        assert match is not None, text
        return Color(
            red=int(match.group(1), 16),
            green=int(match.group(2), 16),
            blue=int(match.group(3), 16),
            brightness=int(match.group(4), 16),
            blink_ms=int(match.group(5), 16),
        )


@dataclasses.dataclass(frozen=True)
class Colors:
    ROLES = frozenset(["inactive", "active", "flashing", "beacon", "bootloader"])

    inactive:   Color = dataclasses.field(default=Color(255, 0,   0,   64,  0))
    active:     Color = dataclasses.field(default=Color(0,   255, 0,   128, 0))
    flashing:   Color = dataclasses.field(default=Color(0,   170, 255, 128, 0))
    beacon:     Color = dataclasses.field(default=Color(228, 44,  156, 255, 250))
    bootloader: Color = dataclasses.field(default=Color(255, 170, 0,   128, 0))
    crc:        int = dataclasses.field(default=0)
    _packed:    bytes = dataclasses.field(default=b"")

    __crc_struct = struct.Struct("<HHHHH")

    def __post_init__(self) -> None:
        crcs: list[int] = []
        packed: bytes = b""
        for color in [self.inactive, self.active, self.flashing, self.beacon, self.bootloader]:
            crcs.append(color.crc)
            packed += color.pack()
        object.__setattr__(self, "crc", bitbang.make_crc16(self.__crc_struct.pack(*crcs)))
        object.__setattr__(self, "_packed", packed)

    def pack(self) -> bytes:
        return self._packed


# =====
_T = TypeVar("_T")


class _PortsDict(Generic[_T]):
    def __init__(self, default: _T, kvs: dict[int, _T]) -> None:
        self.default = default
        self.kvs = {
            port: value
            for (port, value) in kvs.items()
            if value != default
        }

    def compare_on_ports(self, other: "_PortsDict[_T]", ports: int) -> bool:
        for port in range(ports):
            if self[port] != other[port]:
                return False
        return True

    def __getitem__(self, port: int) -> _T:
        return self.kvs.get(port, self.default)

    def __setitem__(self, port: int, value: (_T | None)) -> None:
        if value is None:
            value = self.default
        if value == self.default:
            self.kvs.pop(port, None)
        else:
            self.kvs[port] = value


class PortNames(_PortsDict[str]):
    def __init__(self, kvs: dict[int, str]) -> None:
        super().__init__("", kvs)

    def copy(self) -> "PortNames":
        return PortNames(self.kvs)


class AtxClickPowerDelays(_PortsDict[float]):
    def __init__(self, kvs: dict[int, float]) -> None:
        super().__init__(0.5, kvs)

    def copy(self) -> "AtxClickPowerDelays":
        return AtxClickPowerDelays(self.kvs)


class AtxClickPowerLongDelays(_PortsDict[float]):
    def __init__(self, kvs: dict[int, float]) -> None:
        super().__init__(5.5, kvs)

    def copy(self) -> "AtxClickPowerLongDelays":
        return AtxClickPowerLongDelays(self.kvs)


class AtxClickResetDelays(_PortsDict[float]):
    def __init__(self, kvs: dict[int, float]) -> None:
        super().__init__(0.5, kvs)

    def copy(self) -> "AtxClickResetDelays":
        return AtxClickResetDelays(self.kvs)
