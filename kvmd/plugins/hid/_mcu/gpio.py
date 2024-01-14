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

        self.__line_request: (gpiod.LineRequest | None) = None
        self.__last_power: (bool | None) = None

    def __enter__(self) -> None:
        if self.__power_detect_pin >= 0 or self.__reset_pin >= 0:
            assert self.__line_request is None
            config: dict[int, gpiod.LineSettings] = {}
            if self.__power_detect_pin >= 0:
                config[self.__power_detect_pin] = gpiod.LineSettings(
                    direction=gpiod.line.Direction.INPUT,
                    bias=(gpiod.line.Bias.PULL_DOWN if self.__power_detect_pull_down else gpiod.line.Bias.AS_IS),
                )
            if self.__reset_pin >= 0:
                config[self.__reset_pin] = gpiod.LineSettings(
                    direction=gpiod.line.Direction.OUTPUT,
                    output_value=gpiod.line.Value(self.__reset_inverted),
                )
            assert len(config) > 0
            self.__line_request = gpiod.request_lines(
                self.__device_path,
                consumer="kvmd::hid",
                config=config,
            )

    def __exit__(
        self,
        _exc_type: type[BaseException],
        _exc: BaseException,
        _tb: types.TracebackType,
    ) -> None:

        if self.__line_request:
            try:
                self.__line_request.release()
            except Exception:
                pass
            self.__last_power = None
            self.__line_request = None

    def is_powered(self) -> bool:
        if self.__power_detect_pin >= 0:
            assert self.__line_request
            power = bool(self.__line_request.get_value(self.__power_detect_pin).value)
            if power != self.__last_power:
                get_logger(0).info("HID power state changed: %s -> %s", self.__last_power, power)
                self.__last_power = power
            return power
        return True

    def reset(self) -> None:
        if self.__reset_pin >= 0:
            assert self.__line_request
            try:
                self.__line_request.set_value(self.__reset_pin, gpiod.line.Value(not self.__reset_inverted))
                time.sleep(self.__reset_delay)
            finally:
                self.__line_request.set_value(self.__reset_pin, gpiod.line.Value(self.__reset_inverted))
                time.sleep(1)
            get_logger(0).info("Reset HID performed")
