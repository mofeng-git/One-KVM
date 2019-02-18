# ========================================================================== #
#                                                                            #
#    KVMD - The The main Pi-KVM daemon.                                      #
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


import asyncio

from typing import Dict
from typing import AsyncGenerator

from ...logging import get_logger

from ... import aioregion
from ... import gpio


# =====
class AtxIsBusy(aioregion.RegionIsBusyError):
    pass


class Atx:  # pylint: disable=too-many-instance-attributes
    def __init__(
        self,
        power_led_pin: int,
        hdd_led_pin: int,

        power_switch_pin: int,
        reset_switch_pin: int,
        click_delay: float,
        long_click_delay: float,

        state_poll: float,
    ) -> None:

        self.__power_led_pin = gpio.set_input(power_led_pin)
        self.__hdd_led_pin = gpio.set_input(hdd_led_pin)

        self.__power_switch_pin = gpio.set_output(power_switch_pin)
        self.__reset_switch_pin = gpio.set_output(reset_switch_pin)
        self.__click_delay = click_delay
        self.__long_click_delay = long_click_delay

        self.__state_poll = state_poll

        self.__region = aioregion.AioExclusiveRegion(AtxIsBusy)

    def get_state(self) -> Dict:
        return {
            "busy": self.__region.is_busy(),
            "leds": {
                "power": (not gpio.read(self.__power_led_pin)),
                "hdd": (not gpio.read(self.__hdd_led_pin)),
            },
        }

    async def poll_state(self) -> AsyncGenerator[Dict, None]:
        while True:
            yield self.get_state()
            await asyncio.sleep(self.__state_poll)

    async def click_power(self) -> None:
        get_logger().info("Clicking power ...")
        await self.__click(self.__power_switch_pin, self.__click_delay)

    async def click_power_long(self) -> None:
        get_logger().info("Clicking power (long press) ...")
        await self.__click(self.__power_switch_pin, self.__long_click_delay)

    async def click_reset(self) -> None:
        get_logger().info("Clicking reset")
        await self.__click(self.__reset_switch_pin, self.__click_delay)

    async def __click(self, pin: int, delay: float) -> None:
        self.__region.enter()
        asyncio.ensure_future(self.__inner_click(pin, delay))

    async def __inner_click(self, pin: int, delay: float) -> None:
        try:
            for flag in (True, False):
                gpio.write(pin, flag)
                await asyncio.sleep(delay)
        finally:
            self.__region.exit()
