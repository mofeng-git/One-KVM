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


from typing import Optional

import gpiod

from ....logging import get_logger

from .... import env
from .... import aiotools
from .... import aiogp


# =====
class Gpio:
    def __init__(self, reset_pin: int, reset_delay: float) -> None:
        self.__reset_pin = reset_pin
        self.__reset_delay = reset_delay

        self.__chip: Optional[gpiod.Chip] = None
        self.__reset_line: Optional[gpiod.Line] = None
        self.__reset_wip = False

    def open(self) -> None:
        if self.__reset_pin >= 0:
            assert self.__chip is None
            assert self.__reset_line is None
            self.__chip = gpiod.Chip(env.GPIO_DEVICE_PATH)
            self.__reset_line = self.__chip.get_line(self.__reset_pin)
            self.__reset_line.request("kvmd::hid-mcu::reset", gpiod.LINE_REQ_DIR_OUT, default_vals=[0])

    def close(self) -> None:
        if self.__chip:
            try:
                self.__chip.close()
            except Exception:
                pass

    @aiotools.atomic
    async def reset(self) -> None:
        if self.__reset_pin >= 0:
            assert self.__reset_line
            if not self.__reset_wip:
                self.__reset_wip = True
                try:
                    await aiogp.pulse(self.__reset_line, self.__reset_delay, 1)
                finally:
                    self.__reset_wip = False
                get_logger(0).info("Reset HID performed")
            else:
                get_logger(0).info("Another reset HID in progress")
