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
from typing import Set
from typing import Optional

from ... import aiotools
from ... import gpio

from ...yamlconf import Option

from ...validators.basic import valid_float_f01

from . import BaseUserGpioDriver


# =====
class Plugin(BaseUserGpioDriver):
    def __init__(
        self,
        instance_name: str,
        notifier: aiotools.AioNotifier,

        state_poll: float,
    ) -> None:

        super().__init__(instance_name, notifier)

        self.__state_poll = state_poll

        self.__input_pins: Set[int] = set()
        self.__output_pins: Dict[int, Optional[bool]] = {}

        self.__reader: Optional[gpio.BatchReader] = None

    @classmethod
    def get_plugin_options(cls) -> Dict:
        return {
            "state_poll": Option(0.1, type=valid_float_f01),
        }

    def register_input(self, pin: int) -> None:
        self.__input_pins.add(pin)

    def register_output(self, pin: int, initial: Optional[bool]) -> None:
        self.__output_pins[pin] = initial

    def prepare(self) -> None:
        assert self.__reader is None
        self.__reader = gpio.BatchReader(
            pins=set([
                *map(gpio.set_input, self.__input_pins),
                *[
                    gpio.set_output(pin, initial)
                    for (pin, initial) in self.__output_pins.items()
                ],
            ]),
            interval=self.__state_poll,
            notifier=self._notifier,
        )

    async def run(self) -> None:
        assert self.__reader
        await self.__reader.poll()

    def read(self, pin: int) -> bool:
        return gpio.read(pin)

    def write(self, pin: int, state: bool) -> None:
        gpio.write(pin, state)

    def __str__(self) -> str:
        return f"GPIO({self._instance_name})"

    __repr__ = __str__
