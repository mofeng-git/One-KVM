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


from typing import Callable
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

        self.__input_pins: dict[int, aiogp.AioReaderPinParams] = {}
        self.__output_pins: dict[int, (bool | None)] = {}

        self.__reader: (aiogp.AioReader | None) = None
        self.__outputs_request: (gpiod.LineRequest | None) = None

    @classmethod
    def get_plugin_options(cls) -> dict:
        return {
            "device": Option("/dev/gpiochip0", type=valid_abs_path, unpack_as="device_path"),
        }

    @classmethod
    def get_pin_validator(cls) -> Callable[[Any], Any]:
        return valid_gpio_pin

    def register_input(self, pin: str, debounce: float) -> None:
        self.__input_pins[int(pin)] = aiogp.AioReaderPinParams(False, debounce)

    def register_output(self, pin: str, initial: (bool | None)) -> None:
        self.__output_pins[int(pin)] = initial

    def prepare(self) -> None:
        assert self.__reader is None
        assert self.__outputs_request is None
        self.__reader = aiogp.AioReader(
            path=self.__device_path,
            consumer="kvmd::gpio::inputs",
            pins=self.__input_pins,
            notifier=self._notifier,
        )
        if self.__output_pins:
            self.__outputs_request = gpiod.request_lines(
                self.__device_path,
                consumer="kvmd::gpiod::outputs",
                config={
                    pin: gpiod.LineSettings(
                        direction=gpiod.line.Direction.OUTPUT,
                        output_value=gpiod.line.Value(initial or False),
                    )
                    for (pin, initial) in self.__output_pins.items()
                },
            )

    async def run(self) -> None:
        assert self.__reader
        await self.__reader.poll()

    async def cleanup(self) -> None:
        if self.__outputs_request:
            try:
                self.__outputs_request.release()
            except Exception:
                pass

    async def read(self, pin: str) -> bool:
        assert self.__reader
        pin_int = int(pin)
        if pin_int in self.__input_pins:
            return self.__reader.get(pin_int)
        assert self.__outputs_request
        assert pin_int in self.__output_pins
        return bool(self.__outputs_request.get_value(pin_int).value)

    async def write(self, pin: str, state: bool) -> None:
        assert self.__outputs_request
        pin_int = int(pin)
        assert pin_int in self.__output_pins
        self.__outputs_request.set_value(pin_int, gpiod.line.Value(state))

    def __str__(self) -> str:
        return f"GPIO({self._instance_name})"

    __repr__ = __str__
