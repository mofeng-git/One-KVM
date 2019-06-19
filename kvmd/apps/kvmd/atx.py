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


import asyncio
import operator

from typing import Dict
from typing import Callable
from typing import AsyncGenerator
from typing import Any

from ...logging import get_logger

from ... import aiotools
from ... import aioregion
from ... import gpio


# =====
class AtxError(Exception):
    pass


class AtxOperationError(AtxError):
    pass


class AtxDisabledError(AtxOperationError):
    def __init__(self) -> None:
        super().__init__("ATX is disabled")


class AtxIsBusyError(AtxOperationError, aioregion.RegionIsBusyError):
    pass


def _atx_working(method: Callable) -> Callable:
    async def wrapper(self: "Atx", *args: Any, **kwargs: Any) -> Any:
        if not self._enabled:  # pylint: disable=protected-access
            raise AtxDisabledError()
        return (await method(self, *args, **kwargs))
    return wrapper


class Atx:  # pylint: disable=too-many-instance-attributes
    def __init__(  # pylint: disable=too-many-arguments
        self,
        enabled: bool,

        power_led_pin: int,
        hdd_led_pin: int,
        power_led_inverted: bool,
        hdd_led_inverted: bool,

        power_switch_pin: int,
        reset_switch_pin: int,
        click_delay: float,
        long_click_delay: float,

        state_poll: float,
    ) -> None:

        self._enabled = enabled

        if self._enabled:
            self.__power_led_pin = gpio.set_input(power_led_pin)
            self.__hdd_led_pin = gpio.set_input(hdd_led_pin)
            self.__power_switch_pin = gpio.set_output(power_switch_pin)
            self.__reset_switch_pin = gpio.set_output(reset_switch_pin)
        else:
            self.__power_led_pin = -1
            self.__hdd_led_pin = -1
            self.__power_switch_pin = -1
            self.__reset_switch_pin = -1

        self.__power_led_inverted = power_led_inverted
        self.__hdd_led_inverted = hdd_led_inverted

        self.__click_delay = click_delay
        self.__long_click_delay = long_click_delay

        self.__state_poll = state_poll

        self.__region = aioregion.AioExclusiveRegion(AtxIsBusyError)

    def get_state(self) -> Dict:
        if self._enabled:
            power_led_state = operator.xor(self.__power_led_inverted, gpio.read(self.__power_led_pin))
            hdd_led_state = operator.xor(self.__hdd_led_inverted, gpio.read(self.__hdd_led_pin))
        else:
            power_led_state = hdd_led_state = False
        return {
            "enabled": self._enabled,
            "busy": self.__region.is_busy(),
            "leds": {
                "power": power_led_state,
                "hdd": hdd_led_state,
            },
        }

    async def poll_state(self) -> AsyncGenerator[Dict, None]:
        prev_state: Dict = {}
        while True:
            if self._enabled:
                state = self.get_state()
                if state != prev_state:
                    yield state
                    prev_state = state
                await asyncio.sleep(self.__state_poll)
            else:
                await asyncio.sleep(60)

    async def cleanup(self) -> None:
        for (name, pin) in [
            ("power", self.__power_switch_pin),
            ("reset", self.__reset_switch_pin),
        ]:
            try:
                gpio.write(pin, False)
            except Exception:
                get_logger(0).exception("Can't cleanup %s pin %d", name, pin)

    # =====

    @_atx_working
    async def power_on(self) -> bool:
        if not self.get_state()["leds"]["power"]:
            await self.click_power()
            return True
        return False

    @_atx_working
    async def power_off(self) -> bool:
        if self.get_state()["leds"]["power"]:
            await self.click_power()
            return True
        return False

    @_atx_working
    async def power_off_hard(self) -> bool:
        if self.get_state()["leds"]["power"]:
            await self.click_power_long()
            return True
        return False

    @_atx_working
    async def power_reset_hard(self) -> bool:
        if self.get_state()["leds"]["power"]:
            await self.click_reset()
            return True
        return False

    # =====

    @_atx_working
    async def click_power(self) -> None:
        await self.__click("power", self.__power_switch_pin, self.__click_delay)

    @_atx_working
    async def click_power_long(self) -> None:
        await self.__click("power_long", self.__power_switch_pin, self.__long_click_delay)

    @_atx_working
    async def click_reset(self) -> None:
        await self.__click("reset", self.__reset_switch_pin, self.__click_delay)

    # =====

    @aiotools.atomic
    async def __click(self, name: str, pin: int, delay: float) -> None:
        with aiotools.unregion_only_on_exception(self.__region):
            await self.__inner_click(name, pin, delay)

    @aiotools.tasked
    @aiotools.muted("Can't perform ATX click or operation was not completed")
    async def __inner_click(self, name: str, pin: int, delay: float) -> None:
        try:
            gpio.write(pin, True)
            await asyncio.sleep(delay)
        finally:
            try:
                gpio.write(pin, False)
                await asyncio.sleep(1)
            finally:
                self.__region.exit()
        get_logger(0).info("Clicked ATX button %r", name)
