# ========================================================================== #
#                                                                            #
#    KVMD - The main PiKVM daemon.                                           #
#                                                                            #
#    Copyright (C) 2018-2021  Maxim Devaev <mdevaev@gmail.com>               #
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


import types
import time

from typing import Type
from typing import Optional

import gpiod

from ....logging import get_logger


# =====
class Gpio:
    def __init__(
        self,
        device_path: str,
        reset_pin: int,
        reset_inverted: bool,
        reset_delay: float,
    ) -> None:

        self.__device_path = device_path
        self.__reset_pin = reset_pin
        self.__reset_inverted = reset_inverted
        self.__reset_delay = reset_delay

        self.__chip: Optional[gpiod.Chip] = None
        self.__reset_line: Optional[gpiod.Line] = None

    def __enter__(self) -> None:
        if self.__reset_pin >= 0:
            assert self.__chip is None
            assert self.__reset_line is None
            self.__chip = gpiod.Chip(self.__device_path)
            self.__reset_line = self.__chip.get_line(self.__reset_pin)
            self.__reset_line.request("kvmd::hid::reset", gpiod.LINE_REQ_DIR_OUT, default_vals=[int(self.__reset_inverted)])

    def __exit__(
        self,
        _exc_type: Type[BaseException],
        _exc: BaseException,
        _tb: types.TracebackType,
    ) -> None:

        if self.__chip:
            try:
                self.__chip.close()
            except Exception:
                pass
            self.__reset_line = None
            self.__chip = None

    def reset(self) -> None:
        if self.__reset_pin >= 0:
            assert self.__reset_line
            try:
                self.__reset_line.set_value(int(not self.__reset_inverted))
                time.sleep(self.__reset_delay)
            finally:
                self.__reset_line.set_value(int(self.__reset_inverted))
                time.sleep(1)
            get_logger(0).info("Reset HID performed")
