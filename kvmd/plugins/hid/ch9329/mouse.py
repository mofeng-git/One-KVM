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


class Mouse:  # pylint: disable=too-many-instance-attributes
    def __init__(self) -> None:
        self.__active = "usb"
        self.__buttons = 0x00
        self.__to_x = [0, 0]
        self.__to_y = [0, 0]
        self.__wheel_y = 0
        self.__delta_x = 0
        self.__delta_y = 0

    def button(self, button: str, clicked: bool) -> list[int]:
        code = self.__button_code(button)
        if code and self.__buttons:
            self.__buttons &= ~code
        if clicked:
            self.__buttons |= code
        self.__wheel_y = 0
        if self.__active != "usb":
            self.__to_x = [0, 0]
            self.__to_y = [0, 0]
        return self.__absolute()

    def move(self, to_x: int, to_y: int) -> list[int]:
        assert MouseRange.MIN <= to_x <= MouseRange.MAX
        assert MouseRange.MIN <= to_y <= MouseRange.MAX
        self.__to_x = self.__to_fixed(to_x)
        self.__to_y = self.__to_fixed(to_y)
        self.__wheel_y = 0
        return self.__absolute()

    def wheel(self, delta_x: int, delta_y: int) -> list[int]:
        assert -127 <= delta_y <= 127
        _ = delta_x
        self.__wheel_y = 1 if delta_y > 0 else 255
        return self.__absolute()

    def relative(self, delta_x: int, delta_y: int) -> list[int]:
        assert -127 <= delta_x <= 127
        assert -127 <= delta_y <= 127
        delta_x = math.ceil(delta_x / 3)
        delta_y = math.ceil(delta_y / 3)
        self.__delta_x = delta_x if delta_x >= 0 else 255 + delta_x
        self.__delta_y = delta_y if delta_y >= 0 else 255 + delta_y
        return self.__relative()

    def active(self) -> str:
        return self.__active

    def set_active(self, name: str) -> None:
        self.__active = name

    def __absolute(self) -> list[int]:
        cmd = [
            0x00, 0x04, 0x07, 0x02,
            self.__buttons,
            self.__to_x[1], self.__to_x[0],
            self.__to_y[1], self.__to_y[0],
            0x00]
        if self.__wheel_y:
            cmd[9] = self.__wheel_y
        return cmd

    def __relative(self) -> list[int]:
        cmd = [
            0x00, 0x05, 0x05, 0x01,
            self.__buttons,
            self.__delta_x, self.__delta_y,
            0x00]
        return cmd

    def __button_code(self, name: str) -> int:
        code = 0x00
        match name:
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
        return code

    def __to_fixed(self, num: int) -> list[int]:
        to_fixed = math.ceil(MouseRange.remap(num, 0, MouseRange.MAX) / 8)
        return [to_fixed >> 8, to_fixed & 0xFF]
