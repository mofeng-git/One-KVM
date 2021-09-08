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


from typing import Dict
from typing import Callable
from typing import Optional
from typing import Any

import gpiod

from ... import aiotools
from ... import aiogp

from ...yamlconf import Option

from ...validators.os import valid_abs_path
from ...validators.hw import valid_gpio_pin

from . import BaseUserGpioDriver


# =====
class Plugin(BaseUserGpioDriver):
    def __init__(
        self,
        instance_name: str,
        notifier: aiotools.AioNotifier,

        device_path: str,
    ) -> None:

        super().__init__(instance_name, notifier)

        self.__device_path = device_path

        self.__input_pins: Dict[int, aiogp.AioReaderPinParams] = {}
        self.__output_pins: Dict[int, Optional[bool]] = {}

        self.__reader: Optional[aiogp.AioReader] = None

        self.__chip: Optional[gpiod.Chip] = None
        self.__output_lines: Dict[int, gpiod.Line] = {}

    @classmethod
    def get_plugin_options(cls) -> Dict:
        return {
            "device": Option("/dev/gpiochip0", type=valid_abs_path, unpack_as="device_path"),
        }

    @classmethod
    def get_pin_validator(cls) -> Callable[[Any], Any]:
        return valid_gpio_pin

    def register_input(self, pin: str, debounce: float) -> None:
        self.__input_pins[int(pin)] = aiogp.AioReaderPinParams(False, debounce)

    def register_output(self, pin: str, initial: Optional[bool]) -> None:
        self.__output_pins[int(pin)] = initial

    def prepare(self) -> None:
        assert self.__reader is None
        self.__reader = aiogp.AioReader(
            path=self.__device_path,
            consumer="kvmd::gpio::inputs",
            pins=self.__input_pins,
            notifier=self._notifier,
        )

        self.__chip = gpiod.Chip(self.__device_path)
        for (pin, initial) in self.__output_pins.items():
            line = self.__chip.get_line(pin)
            line.request("kvmd::gpio::outputs", gpiod.LINE_REQ_DIR_OUT, default_vals=[int(initial or False)])
            self.__output_lines[pin] = line

    async def run(self) -> None:
        assert self.__reader
        await self.__reader.poll()

    async def cleanup(self) -> None:
        if self.__chip:
            try:
                self.__chip.close()
            except Exception:
                pass

    async def read(self, pin: str) -> bool:
        assert self.__reader
        pin_int = int(pin)
        if pin_int in self.__input_pins:
            return self.__reader.get(pin_int)
        return bool(self.__output_lines[pin_int].get_value())

    async def write(self, pin: str, state: bool) -> None:
        self.__output_lines[int(pin)].set_value(int(state))

    def __str__(self) -> str:
        return f"GPIO({self._instance_name})"

    __repr__ = __str__
