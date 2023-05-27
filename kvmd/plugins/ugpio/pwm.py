# ========================================================================== #
#                                                                            #
#    KVMD - The main PiKVM daemon.                                           #
#                                                                            #
#    Copyright (C) 2018-2023  Maxim Devaev <mdevaev@gmail.com>               #
#                             Shantur Rathore <i@shantur.com>                #
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

from periphery import PWM

from ...logging import get_logger

from ... import tools
from ... import aiotools

from ...yamlconf import Option

from ...validators.basic import valid_int_f0
from ...validators.hw import valid_gpio_pin

from . import GpioDriverOfflineError
from . import UserGpioModes
from . import BaseUserGpioDriver


# =====
class Plugin(BaseUserGpioDriver):
    def __init__(  # pylint: disable=super-init-not-called
        self,
        instance_name: str,
        notifier: aiotools.AioNotifier,

        chip: int,
        period: int,
        duty_cycle_push: int,
        duty_cycle_release: int,
    ) -> None:

        super().__init__(instance_name, notifier)

        self.__chip = chip
        self.__period = period
        self.__duty_cycle_push = duty_cycle_push
        self.__duty_cycle_release = duty_cycle_release

        self.__channels: dict[int, (bool | None)] = {}
        self.__pwms: dict[int, PWM] = {}

    @classmethod
    def get_plugin_options(cls) -> dict:
        return {
            "chip":               Option(0,        type=valid_int_f0),
            "period":             Option(20000000, type=valid_int_f0),
            "duty_cycle_push":    Option(1500000,  type=valid_int_f0),
            "duty_cycle_release": Option(1000000,  type=valid_int_f0),
        }

    @classmethod
    def get_modes(cls) -> set[str]:
        return set([UserGpioModes.OUTPUT])

    @classmethod
    def get_pin_validator(cls) -> Callable[[Any], Any]:
        return valid_gpio_pin

    def register_output(self, pin: str, initial: (bool | None)) -> None:
        self.__channels[int(pin)] = initial

    def prepare(self) -> None:
        logger = get_logger(0)
        for (pin, initial) in self.__channels.items():
            try:
                logger.info("Probing pwm chip %d channel %d ...", self.__chip, pin)
                pwm = PWM(self.__chip, pin)
                self.__pwms[pin] = pwm
                pwm.period_ns = self.__period
                pwm.duty_cycle_ns = self.__get_duty_cycle(bool(initial))
                pwm.enable()
            except Exception as err:
                logger.error("Can't get PWM chip %d channel %d: %s",
                             self.__chip, pin, tools.efmt(err))

    async def cleanup(self) -> None:
        for (pin, pwm) in self.__pwms.items():
            try:
                pwm.disable()
                pwm.close()
            except Exception as err:
                get_logger(0).error("Can't cleanup PWM chip %d channel %d: %s",
                                    self.__chip, pin, tools.efmt(err))

    async def read(self, pin: str) -> bool:
        try:
            return (self.__pwms[int(pin)].duty_cycle_ns == self.__duty_cycle_push)
        except Exception:
            raise GpioDriverOfflineError(self)

    async def write(self, pin: str, state: bool) -> None:
        try:
            self.__pwms[int(pin)].duty_cycle_ns = self.__get_duty_cycle(state)
        except Exception:
            raise GpioDriverOfflineError(self)

    def __get_duty_cycle(self, state: bool) -> int:
        return (self.__duty_cycle_push if state else self.__duty_cycle_release)

    def __str__(self) -> str:
        return f"PWM({self._instance_name})"

    __repr__ = __str__
