# ========================================================================== #
#                                                                            #
#    KVMD - The main Pi-KVM daemon.                                          #
#                                                                            #
#    Copyright (C) 2018-2021  Maxim Devaev <mdevaev@gmail.com>               #
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

from periphery import PWM

from typing import Dict
from typing import Optional
from typing import Set

from ...logging import get_logger

from ... import tools
from ... import aiotools

from ...yamlconf import Option

from ...validators.basic import valid_int_f0

from . import GpioDriverOfflineError
from . import UserGpioModes
from . import BaseUserGpioDriver


# =====
class Plugin(BaseUserGpioDriver):

    def __init__(  # pylint: disable=super-init-not-called
        self,
        instance_name: str,
        notifier: aiotools.AioNotifier,

        pwm_chip: int,
        pwm_period: int,
        duty_cycle_push: int,
        duty_cycle_release: int,
    ) -> None:

        super().__init__(instance_name, notifier)

        self.__pwm_chip = pwm_chip
        self.__pwm_period = pwm_period
        self.__duty_cycle_push = duty_cycle_push
        self.__duty_cycle_release = duty_cycle_release

        self.__channels: Dict[int, Optional[bool]] = {}

        self.__channel_pwm: Dict[int, PWM] = {}

    @classmethod
    def get_plugin_options(cls) -> Dict:
        return {
            "pwm_chip": Option(0, type=valid_int_f0),
            "pwm_period": Option(20000000, type=valid_int_f0),
            "duty_cycle_push": Option(1500000, type=valid_int_f0),
            "duty_cycle_release": Option(1000000, type=valid_int_f0),
        }

    @classmethod
    def get_modes(cls) -> Set[str]:
        return set([UserGpioModes.OUTPUT])

    def register_input(self, pin: int, debounce: float) -> None:
        raise RuntimeError(f"Unsupported mode 'input' for pin={pin} on {self}")

    def register_output(self, pin: int, initial: Optional[bool]) -> None:
        self.__channels[pin] = initial

    def prepare(self) -> None:
        logger = get_logger(0)

        for (pin, initial) in self.__channels.items():
            try:
                logger.info("Probing pwm chip %d channel %d ...", self.__pwm_chip, pin)
                pwm = PWM(self.__pwm_chip, pin)
                self.__channel_pwm[pin] = pwm
                pwm.period_ns = self.__pwm_period
                pwm.duty_cycle_ns = self.__duty_cycle_push if initial else self.__duty_cycle_release
                pwm.enable()

            except Exception as err:
                logger.error("Can't get pwm chip %d channel %d: %s",
                             self.__pwm_chip, pin, tools.efmt(err))

    async def run(self) -> None:
        await aiotools.wait_infinite()

    async def cleanup(self) -> None:
        for (pin, _) in self.__channels.items():
            self.__channel_pwm[pin].disable()
            self.__channel_pwm[pin].close()

    async def read(self, pin: int) -> bool:
        try:
            return self.__channel_pwm[pin].duty_cycle_ns == self.__duty_cycle_push
        except Exception:
            raise GpioDriverOfflineError(self)

    async def write(self, pin: int, state: bool) -> None:
        try:
            self.__channel_pwm[pin].duty_cycle_ns = self.__duty_cycle_push if state else self.__duty_cycle_release
        except Exception:
            raise GpioDriverOfflineError(self)

    def __str__(self) -> str:
        return f"PWM({self._instance_name})"

    __repr__ = __str__
