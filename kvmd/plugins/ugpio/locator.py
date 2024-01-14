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


import asyncio

from typing import Callable
from typing import Any

import gpiod

from ... import aiotools

from ...yamlconf import Option

from ...validators.os import valid_abs_path
from ...validators.hw import valid_gpio_pin

from . import UserGpioModes
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

        self.__tasks: dict[int, (asyncio.Task | None)] = {}
        self.__line_request: (gpiod.LineRequest | None) = None

    @classmethod
    def get_plugin_options(cls) -> dict:
        return {
            "device": Option("/dev/gpiochip0", type=valid_abs_path, unpack_as="device_path"),
        }

    @classmethod
    def get_modes(cls) -> set[str]:
        return set([UserGpioModes.OUTPUT])

    @classmethod
    def get_pin_validator(cls) -> Callable[[Any], Any]:
        return valid_gpio_pin

    def register_output(self, pin: str, initial: (bool | None)) -> None:
        _ = initial
        self.__tasks[int(pin)] = None

    def prepare(self) -> None:
        self.__line_request = gpiod.request_lines(
            self.__device_path,
            consumer="kvmd::locator",
            config={
                tuple(self.__tasks): gpiod.LineSettings(
                    direction=gpiod.line.Direction.OUTPUT,
                    output_value=gpiod.line.Value(False),
                ),
            },
        )

    async def cleanup(self) -> None:
        tasks = [
            task
            for task in self.__tasks.values()
            if task is not None
        ]
        for task in tasks:
            task.cancel()
        await asyncio.gather(*tasks, return_exceptions=True)
        if self.__line_request:
            try:
                self.__line_request.release()
            except Exception:
                pass

    async def read(self, pin: str) -> bool:
        return (self.__tasks[int(pin)] is not None)

    async def write(self, pin: str, state: bool) -> None:
        pin_int = int(pin)
        task = self.__tasks[pin_int]
        if state and task is None:
            self.__tasks[pin_int] = asyncio.create_task(self.__blink(pin_int))
        elif not state and task is not None:
            task.cancel()
            await task
            self.__tasks[pin_int] = None

    async def __blink(self, pin: int) -> None:
        assert pin in self.__tasks
        assert self.__line_request
        try:
            state = True
            while True:
                self.__line_request.set_value(pin, gpiod.line.Value(state))
                state = (not state)
                await asyncio.sleep(0.1)
        except asyncio.CancelledError:
            pass
        finally:
            self.__line_request.set_value(pin, gpiod.line.Value(False))

    def __str__(self) -> str:
        return f"Locator({self._instance_name})"

    __repr__ = __str__
