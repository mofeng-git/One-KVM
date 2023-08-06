# ========================================================================== #
#                                                                            #
#    KVMD - The main PiKVM daemon.                                           #
#                                                                            #
#    Copyright (C) 2018-2023  Maxim Devaev <mdevaev@gmail.com>               #
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

import gpiod

from ....logging import get_logger


# =====
class Gpio:  # pylint: disable=too-many-instance-attributes
    def __init__(
        self,
        device_path: str,
        power_detect_pin: int,
        power_detect_pull_down: bool,
        reset_pin: int,
        reset_inverted: bool,
        reset_delay: float,
    ) -> None:

        self.__device_path = device_path
        self.__power_detect_pin = power_detect_pin
        self.__power_detect_pull_down = power_detect_pull_down
        self.__reset_pin = reset_pin
        self.__reset_inverted = reset_inverted
        self.__reset_delay = reset_delay

        self.__chip: (gpiod.Chip | None) = None
        self.__power_detect_line: (gpiod.Line | None) = None
        self.__reset_line: (gpiod.Line | None) = None

        self.__last_power: (bool | None) = None

    def __enter__(self) -> None:
        if self.__power_detect_pin >= 0 or self.__reset_pin >= 0:
            assert self.__chip is None
            self.__chip = gpiod.Chip(self.__device_path)
            if self.__power_detect_pin >= 0:
                assert self.__power_detect_line is None
                self.__power_detect_line = self.__chip.get_line(self.__power_detect_pin)
                self.__power_detect_line.request(
                    "kvmd::hid::power_detect", gpiod.LINE_REQ_DIR_IN,
                    flags=(gpiod.LINE_REQ_FLAG_BIAS_PULL_DOWN if self.__power_detect_pull_down else 0),
                )
            if self.__reset_pin >= 0:
                assert self.__reset_line is None
                self.__reset_line = self.__chip.get_line(self.__reset_pin)
                self.__reset_line.request(
                    "kvmd::hid::reset", gpiod.LINE_REQ_DIR_OUT,
                    default_vals=[int(self.__reset_inverted)],
                )

    def __exit__(
        self,
        _exc_type: type[BaseException],
        _exc: BaseException,
        _tb: types.TracebackType,
    ) -> None:

        if self.__chip:
            try:
                self.__chip.close()
            except Exception:
                pass
            self.__last_power = None
            self.__power_detect_line = None
            self.__reset_line = None
            self.__chip = None

    def is_powered(self) -> bool:
        if self.__power_detect_line is not None:
            power = bool(self.__power_detect_line.get_value())
            if power != self.__last_power:
                get_logger(0).info("HID power state changed: %s -> %s", self.__last_power, power)
                self.__last_power = power
            return power
        return True

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
