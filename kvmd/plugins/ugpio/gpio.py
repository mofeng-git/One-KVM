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


from typing import Dict
from typing import Optional

import gpiod

from ... import env
from ... import aiotools
from ... import aiogp

from . import BaseUserGpioDriver


# =====
class Plugin(BaseUserGpioDriver):
    def __init__(
        self,
        instance_name: str,
        notifier: aiotools.AioNotifier,
    ) -> None:

        super().__init__(instance_name, notifier)

        self.__input_pins: Dict[int, aiogp.AioReaderPinParams] = {}
        self.__output_pins: Dict[int, Optional[bool]] = {}

        self.__reader: Optional[aiogp.AioReader] = None

        self.__chip: Optional[gpiod.Chip] = None
        self.__output_lines: Dict[int, gpiod.Line] = {}

    def register_input(self, pin: int, debounce: float) -> None:
        self.__input_pins[pin] = aiogp.AioReaderPinParams(False, debounce)

    def register_output(self, pin: int, initial: Optional[bool]) -> None:
        self.__output_pins[pin] = initial

    def prepare(self) -> None:
        assert self.__reader is None
        self.__reader = aiogp.AioReader(
            path=env.GPIO_DEVICE_PATH,
            consumer="kvmd::ugpio-gpio::inputs",
            pins=self.__input_pins,
            notifier=self._notifier,
        )

        self.__chip = gpiod.Chip(env.GPIO_DEVICE_PATH)
        for (pin, initial) in self.__output_pins.items():
            line = self.__chip.get_line(pin)
            line.request("kvmd::ugpio-gpio::outputs", gpiod.LINE_REQ_DIR_OUT, default_vals=[int(initial or False)])
            self.__output_lines[pin] = line

    async def run(self) -> None:
        assert self.__reader
        await self.__reader.poll()

    def cleanup(self) -> None:
        if self.__chip:
            try:
                self.__chip.close()
            except Exception:
                pass

    def read(self, pin: int) -> bool:
        assert self.__reader
        if pin in self.__input_pins:
            return self.__reader.get(pin)
        return bool(self.__output_lines[pin].get_value())

    def write(self, pin: int, state: bool) -> None:
        self.__output_lines[pin].set_value(int(state))

    def __str__(self) -> str:
        return f"GPIO({self._instance_name})"

    __repr__ = __str__
