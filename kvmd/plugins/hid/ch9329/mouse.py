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


import math

from ....mouse import MouseRange


# =====
class Mouse:  # pylint: disable=too-many-instance-attributes
    def __init__(self) -> None:
        self.__absolute = True
        self.__buttons = 0
        self.__to_x = (0, 0)
        self.__to_y = (0, 0)
        self.__delta_x = 0
        self.__delta_y = 0
        self.__wheel_y = 0

    def set_absolute(self, flag: bool) -> None:
        self.__absolute = flag

    def is_absolute(self) -> bool:
        return self.__absolute

    def process_button(self, button: str, state: bool) -> bytes:
        code = 0x00
        match button:
            case "left":
                code = 0x01
            case "right":
                code = 0x02
            case "middle":
                code = 0x04
            case "up":
                code = 0x08
            case "down":
                code = 0x10
        if code:
            if state:
                self.__buttons |= code
            else:
                self.__buttons &= ~code
        self.__wheel_y = 0
        if not self.__absolute:
            return self.__make_relative_cmd()
        else:
            return self.__make_absolute_cmd()

    def process_move(self, to_x: int, to_y: int) -> bytes:
        self.__to_x = self.__fix_absolute(to_x)
        self.__to_y = self.__fix_absolute(to_y)
        self.__wheel_y = 0
        return self.__make_absolute_cmd()

    def __fix_absolute(self, value: int) -> tuple[int, int]:
        assert MouseRange.MIN <= value <= MouseRange.MAX
        to_fixed = math.ceil(MouseRange.remap(value, 0, MouseRange.MAX) / 8)
        return (to_fixed >> 8, to_fixed & 0xFF)

    def process_wheel(self, delta_x: int, delta_y: int) -> bytes:
        _ = delta_x
        assert -127 <= delta_y <= 127
        self.__wheel_y = (1 if delta_y > 0 else 255)
        if not self.__absolute:
            return self.__make_relative_cmd()
        else:
            return self.__make_absolute_cmd()

    def process_relative(self, delta_x: int, delta_y: int) -> bytes:
        self.__delta_x = self.__fix_relative(delta_x)
        self.__delta_y = self.__fix_relative(delta_y)
        self.__wheel_y = 0
        return self.__make_relative_cmd()

    def __make_absolute_cmd(self) -> bytes:
        return bytes([
            0, 0x04, 0x07, 0x02,
            self.__buttons,
            self.__to_x[1], self.__to_x[0],
            self.__to_y[1], self.__to_y[0],
            self.__wheel_y,
        ])

    def __make_relative_cmd(self) -> bytes:
        return bytes([
            0, 0x05, 0x05, 0x01,
            self.__buttons,
            self.__delta_x, self.__delta_y,
            self.__wheel_y,
        ])

    def __fix_relative(self, value: int) -> int:
        assert -127 <= value <= 127
        value = math.ceil(value / 3)
        return (value if value >= 0 else (255 + value))
