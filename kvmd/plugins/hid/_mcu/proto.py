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


import dataclasses
import struct

from evdev import ecodes

from ....keyboard.mappings import KEYMAP

from ....mouse import MouseRange
from ....mouse import MouseDelta

from .... import tools
from .... import bitbang


# =====
class BaseEvent:
    def make_request(self) -> bytes:
        raise NotImplementedError


# =====
_KEYBOARD_NAMES_TO_CODES = {
    "disabled": 0b00000000,
    "usb":      0b00000001,
    "ps2":      0b00000011,
}
_KEYBOARD_CODES_TO_NAMES = tools.swapped_kvs(_KEYBOARD_NAMES_TO_CODES)


def get_active_keyboard(outputs: int) -> str:
    return _KEYBOARD_CODES_TO_NAMES.get(outputs & 0b00000111, "disabled")


@dataclasses.dataclass(frozen=True)
class SetKeyboardOutputEvent(BaseEvent):
    keyboard: str

    def __post_init__(self) -> None:
        assert not self.keyboard or self.keyboard in _KEYBOARD_NAMES_TO_CODES

    def make_request(self) -> bytes:
        code = _KEYBOARD_NAMES_TO_CODES.get(self.keyboard, 0)
        return _make_request(struct.pack(">BBxxx", 0x03, code))


# =====
_MOUSE_NAMES_TO_CODES = {
    "disabled":  0b00000000,
    "usb":       0b00001000,
    "usb_rel":   0b00010000,
    "ps2":       0b00011000,
    "usb_win98": 0b00100000,
}
_MOUSE_CODES_TO_NAMES = tools.swapped_kvs(_MOUSE_NAMES_TO_CODES)


def get_active_mouse(outputs: int) -> str:
    return _MOUSE_CODES_TO_NAMES.get(outputs & 0b00111000, "disabled")


@dataclasses.dataclass(frozen=True)
class SetMouseOutputEvent(BaseEvent):
    mouse: str

    def __post_init__(self) -> None:
        assert not self.mouse or self.mouse in _MOUSE_NAMES_TO_CODES

    def make_request(self) -> bytes:
        return _make_request(struct.pack(">BBxxx", 0x04, _MOUSE_NAMES_TO_CODES.get(self.mouse, 0)))


# =====
@dataclasses.dataclass(frozen=True)
class SetConnectedEvent(BaseEvent):
    connected: bool

    def make_request(self) -> bytes:
        return _make_request(struct.pack(">BBxxx", 0x05, int(self.connected)))


# =====
class ClearEvent(BaseEvent):
    def make_request(self) -> bytes:
        return _make_request(b"\x10\x00\x00\x00\x00")


@dataclasses.dataclass(frozen=True)
class KeyEvent(BaseEvent):
    code:  int
    state: bool

    def __post_init__(self) -> None:
        assert self.code in KEYMAP

    def make_request(self) -> bytes:
        code = KEYMAP[self.code].mcu.code
        return _make_request(struct.pack(">BBBxx", 0x11, code, int(self.state)))


@dataclasses.dataclass(frozen=True)
class MouseButtonEvent(BaseEvent):
    code:  int
    state: bool

    def __post_init__(self) -> None:
        assert self.code in [
            ecodes.BTN_LEFT, ecodes.BTN_RIGHT, ecodes.BTN_MIDDLE,
            ecodes.BTN_BACK, ecodes.BTN_FORWARD,
        ]

    def make_request(self) -> bytes:
        (code, state_pressed, is_main) = {
            ecodes.BTN_LEFT:    (0b10000000, 0b00001000, True),
            ecodes.BTN_RIGHT:   (0b01000000, 0b00000100, True),
            ecodes.BTN_MIDDLE:  (0b00100000, 0b00000010, True),
            ecodes.BTN_BACK:    (0b10000000, 0b00001000, False),  # Up
            ecodes.BTN_FORWARD: (0b01000000, 0b00000100, False),  # Down
        }[self.code]
        if self.state:
            code |= state_pressed
        if is_main:
            main_code = code
            extra_code = 0
        else:
            main_code = 0
            extra_code = code
        return _make_request(struct.pack(">BBBxx", 0x13, main_code, extra_code))


@dataclasses.dataclass(frozen=True)
class MouseMoveEvent(BaseEvent):
    to_x: int
    to_y: int

    def __post_init__(self) -> None:
        assert MouseRange.MIN <= self.to_x <= MouseRange.MAX
        assert MouseRange.MIN <= self.to_y <= MouseRange.MAX

    def make_request(self) -> bytes:
        return _make_request(struct.pack(">Bhh", 0x12, self.to_x, self.to_y))


@dataclasses.dataclass(frozen=True)
class MouseRelativeEvent(BaseEvent):
    delta_x: int
    delta_y: int

    def __post_init__(self) -> None:
        assert MouseDelta.MIN <= self.delta_x <= MouseDelta.MAX
        assert MouseDelta.MIN <= self.delta_y <= MouseDelta.MAX

    def make_request(self) -> bytes:
        return _make_request(struct.pack(">Bbbxx", 0x15, self.delta_x, self.delta_y))


@dataclasses.dataclass(frozen=True)
class MouseWheelEvent(BaseEvent):
    delta_x: int
    delta_y: int

    def __post_init__(self) -> None:
        assert MouseDelta.MIN <= self.delta_x <= MouseDelta.MAX
        assert MouseDelta.MIN <= self.delta_y <= MouseDelta.MAX

    def make_request(self) -> bytes:
        # Горизонтальная прокрутка пока не поддерживается
        return _make_request(struct.pack(">Bxbxx", 0x14, self.delta_y))


# =====
def check_response(resp: bytes) -> bool:
    assert len(resp) in (4, 8), resp
    return (bitbang.make_crc16(resp[:-2]) == struct.unpack(">H", resp[-2:])[0])


def _make_request(cmd: bytes) -> bytes:
    assert len(cmd) == 5, cmd
    req = b"\x33" + cmd
    req += struct.pack(">H", bitbang.make_crc16(req))
    assert len(req) == 8, req
    return req


# =====
REQUEST_PING = _make_request(b"\x01\x00\x00\x00\x00")
REQUEST_REPEAT = _make_request(b"\x02\x00\x00\x00\x00")

RESPONSE_LEGACY_OK = b"\x33\x20" + struct.pack(">H", bitbang.make_crc16(b"\x33\x20"))
