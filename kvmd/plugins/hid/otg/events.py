# ========================================================================== #
#                                                                            #
#    KVMD - The main Pi-KVM daemon.                                          #
#                                                                            #
#    Copyright (C) 2018  Maxim Devaev <mdevaev@gmail.com>                    #
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

from typing import Union

from ....keyboard.mappings import OtgKey
from ....keyboard.mappings import KEYMAP


# =====
class BaseEvent:
    pass


class ClearEvent(BaseEvent):
    pass


class ResetEvent(BaseEvent):
    pass


# =====
@dataclasses.dataclass(frozen=True)
class KeyEvent(BaseEvent):
    key: OtgKey
    state: bool

    def __post_init__(self) -> None:
        assert (not self.key.is_modifier)


@dataclasses.dataclass(frozen=True)
class ModifierEvent(BaseEvent):
    modifier: OtgKey
    state: bool

    def __post_init__(self) -> None:
        assert self.modifier.is_modifier


def make_keyboard_event(key: str, state: bool) -> Union[KeyEvent, ModifierEvent]:
    otg_key = KEYMAP[key].otg
    if otg_key.is_modifier:
        return ModifierEvent(otg_key, state)
    return KeyEvent(otg_key, state)


# =====
@dataclasses.dataclass(frozen=True)
class MouseButtonEvent(BaseEvent):
    button: str
    state: bool
    code: int = 0

    def __post_init__(self) -> None:
        object.__setattr__(self, "code", {
            "left":   0x1,
            "right":  0x2,
            "middle": 0x4,
            "up":     0x8,  # Back
            "down":   0x10,  # Forward
        }[self.button])


@dataclasses.dataclass(frozen=True)
class MouseMoveEvent(BaseEvent):
    to_x: int
    to_y: int
    to_fixed_x: int = 0
    to_fixed_y: int = 0

    def __post_init__(self) -> None:
        assert -32768 <= self.to_x <= 32767
        assert -32768 <= self.to_y <= 32767
        object.__setattr__(self, "to_fixed_x", (self.to_x + 32768) // 2)
        object.__setattr__(self, "to_fixed_y", (self.to_y + 32768) // 2)


@dataclasses.dataclass(frozen=True)
class MouseRelativeEvent(BaseEvent):
    delta_x: int
    delta_y: int

    def __post_init__(self) -> None:
        assert -127 <= self.delta_x <= 127
        assert -127 <= self.delta_y <= 127


@dataclasses.dataclass(frozen=True)
class MouseWheelEvent(BaseEvent):
    delta_x: int
    delta_y: int

    def __post_init__(self) -> None:
        assert -127 <= self.delta_x <= 127
        assert -127 <= self.delta_y <= 127
