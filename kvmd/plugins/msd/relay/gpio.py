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


from typing import Optional

import gpiod

from .... import aiogp


# =====
class Gpio:  # pylint: disable=too-many-instance-attributes
    def __init__(
        self,
        device_path: str,
        target_pin: int,
        reset_pin: int,
        reset_inverted: bool,
        reset_delay: float,
    ) -> None:

        self.__device_path = device_path
        self.__target_pin = target_pin
        self.__reset_pin = reset_pin
        self.__reset_inverted = reset_inverted
        self.__reset_delay = reset_delay

        self.__chip: Optional[gpiod.Chip] = None
        self.__target_line: Optional[gpiod.Line] = None
        self.__reset_line: Optional[gpiod.Line] = None

    def open(self) -> None:
        assert self.__chip is None
        assert self.__target_line is None
        assert self.__reset_line is None

        self.__chip = gpiod.Chip(self.__device_path)

        self.__target_line = self.__chip.get_line(self.__target_pin)
        self.__target_line.request("kvmd::msd::target", gpiod.LINE_REQ_DIR_OUT, default_vals=[0])

        self.__reset_line = self.__chip.get_line(self.__reset_pin)
        self.__reset_line.request("kvmd::msd::reset", gpiod.LINE_REQ_DIR_OUT, default_vals=[int(self.__reset_inverted)])

    def close(self) -> None:
        if self.__chip:
            try:
                self.__chip.close()
            except Exception:
                pass

    def switch_to_local(self) -> None:
        assert self.__target_line
        self.__target_line.set_value(0)

    def switch_to_server(self) -> None:
        assert self.__target_line
        self.__target_line.set_value(1)

    async def reset(self) -> None:
        assert self.__reset_line
        await aiogp.pulse(self.__reset_line, self.__reset_delay, 0, self.__reset_inverted)
