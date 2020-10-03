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
from typing import AsyncGenerator
from typing import Optional

import gpiod

from ...logging import get_logger

from ... import env
from ... import aiotools
from ... import aiogp

from ...yamlconf import Option

from ...validators.basic import valid_bool
from ...validators.basic import valid_float_f0
from ...validators.basic import valid_float_f01
from ...validators.hw import valid_gpio_pin

from . import AtxIsBusyError
from . import BaseAtx


# =====
class Plugin(BaseAtx):  # pylint: disable=too-many-instance-attributes
    def __init__(  # pylint: disable=too-many-arguments,super-init-not-called
        self,
        power_led_pin: int,
        power_led_inverted: bool,
        power_led_debounce: float,

        hdd_led_pin: int,
        hdd_led_inverted: bool,
        hdd_led_debounce: float,

        power_switch_pin: int,
        reset_switch_pin: int,
        click_delay: float,
        long_click_delay: float,
    ) -> None:

        self.__power_led_pin = power_led_pin
        self.__hdd_led_pin = hdd_led_pin
        self.__power_switch_pin = power_switch_pin
        self.__reset_switch_pin = reset_switch_pin

        self.__click_delay = click_delay
        self.__long_click_delay = long_click_delay

        self.__notifier = aiotools.AioNotifier()
        self.__region = aiotools.AioExclusiveRegion(AtxIsBusyError, self.__notifier)

        self.__chip: Optional[gpiod.Chip] = None
        self.__power_switch_line: Optional[gpiod.Line] = None
        self.__reset_switch_line: Optional[gpiod.Line] = None

        self.__reader = aiogp.AioReader(
            path=env.GPIO_DEVICE_PATH,
            consumer="kvmd::atx-gpio::leds",
            pins={
                power_led_pin: aiogp.AioReaderPinParams(power_led_inverted, power_led_debounce),
                hdd_led_pin: aiogp.AioReaderPinParams(hdd_led_inverted, hdd_led_debounce),
            },
            notifier=self.__notifier,
        )

    @classmethod
    def get_plugin_options(cls) -> Dict:
        return {
            "power_led_pin":      Option(-1,    type=valid_gpio_pin),
            "power_led_inverted": Option(False, type=valid_bool),
            "power_led_debounce": Option(0.1,   type=valid_float_f0),

            "hdd_led_pin":      Option(-1,    type=valid_gpio_pin),
            "hdd_led_inverted": Option(False, type=valid_bool),
            "hdd_led_debounce": Option(0.1,   type=valid_float_f0),

            "power_switch_pin": Option(-1,  type=valid_gpio_pin),
            "reset_switch_pin": Option(-1,  type=valid_gpio_pin),
            "click_delay":      Option(0.1, type=valid_float_f01),
            "long_click_delay": Option(5.5, type=valid_float_f01),
        }

    def sysprep(self) -> None:
        assert self.__chip is None
        assert self.__power_switch_line is None
        assert self.__reset_switch_line is None

        self.__chip = gpiod.Chip(env.GPIO_DEVICE_PATH)

        self.__power_switch_line = self.__chip.get_line(self.__power_switch_pin)
        self.__power_switch_line.request("kvmd::atx-gpio::power_switch", gpiod.LINE_REQ_DIR_OUT, default_vals=[0])

        self.__reset_switch_line = self.__chip.get_line(self.__reset_switch_pin)
        self.__reset_switch_line.request("kvmd::atx-gpio::reset_switch", gpiod.LINE_REQ_DIR_OUT, default_vals=[0])

    async def get_state(self) -> Dict:
        return {
            "enabled": True,
            "busy": self.__region.is_busy(),
            "leds": {
                "power": self.__reader.get(self.__power_led_pin),
                "hdd": self.__reader.get(self.__hdd_led_pin),
            },
        }

    async def poll_state(self) -> AsyncGenerator[Dict, None]:
        prev_state: Dict = {}
        while True:
            state = await self.get_state()
            if state != prev_state:
                yield state
                prev_state = state
            await self.__notifier.wait()

    async def systask(self) -> None:
        await self.__reader.poll()

    async def cleanup(self) -> None:
        if self.__chip:
            try:
                self.__chip.close()
            except Exception:
                pass

    # =====

    async def power_on(self, wait: bool) -> None:
        if not (await self.__get_power()):
            await self.click_power(wait)

    async def power_off(self, wait: bool) -> None:
        if (await self.__get_power()):
            await self.click_power(wait)

    async def power_off_hard(self, wait: bool) -> None:
        if (await self.__get_power()):
            await self.click_power_long(wait)

    async def power_reset_hard(self, wait: bool) -> None:
        if (await self.__get_power()):
            await self.click_reset(wait)

    # =====

    async def click_power(self, wait: bool) -> None:
        await self.__click("power", self.__power_switch_line, self.__click_delay, wait)

    async def click_power_long(self, wait: bool) -> None:
        await self.__click("power_long", self.__power_switch_line, self.__long_click_delay, wait)

    async def click_reset(self, wait: bool) -> None:
        await self.__click("reset", self.__reset_switch_line, self.__click_delay, wait)

    # =====

    async def __get_power(self) -> bool:
        return (await self.get_state())["leds"]["power"]

    @aiotools.atomic
    async def __click(self, name: str, line: gpiod.Line, delay: float, wait: bool) -> None:
        if wait:
            async with self.__region:
                await self.__inner_click(name, line, delay)
        else:
            await aiotools.run_region_task(
                f"Can't perform ATX {name} click or operation was not completed",
                self.__region, self.__inner_click, name, line, delay,
            )

    @aiotools.atomic
    async def __inner_click(self, name: str, line: gpiod.Line, delay: float) -> None:
        await aiogp.pulse(line, delay, 1)
        get_logger(0).info("Clicked ATX button %r", name)
