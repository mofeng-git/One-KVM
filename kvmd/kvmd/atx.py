import asyncio
import logging

from typing import Tuple

from RPi import GPIO


# =====
_logger = logging.getLogger(__name__)


class Atx:
    def __init__(
        self,
        power_led: int,
        hdd_led: int,
        power_switch: int,
        reset_switch: int,
        click_delay: float,
        long_click_delay: float,
    ) -> None:

        self.__power_led = self.__set_output_pin(power_led)
        self.__hdd_led = self.__set_output_pin(hdd_led)

        self.__power_switch = self.__set_output_pin(power_switch)
        self.__reset_switch = self.__set_output_pin(reset_switch)
        self.__click_delay = click_delay
        self.__long_click_delay = long_click_delay

        self.__lock = asyncio.Lock()

    def __set_output_pin(self, pin: int) -> int:
        GPIO.setup(pin, GPIO.OUT)
        GPIO.output(pin, False)
        return pin

    def get_leds(self) -> Tuple[bool, bool]:
        return (
            not GPIO.input(self.__power_led),
            not GPIO.input(self.__hdd_led),
        )

    async def click_power(self) -> None:
        if (await self.__click(self.__power_switch, self.__click_delay)):
            _logger.info("Clicked power")

    async def click_power_long(self) -> None:
        if (await self.__click(self.__power_switch, self.__long_click_delay)):
            _logger.info("Clicked power (long press)")

    async def click_reset(self) -> None:
        if (await self.__click(self.__reset_switch, self.__click_delay)):
            _logger.info("Clicked reset")

    async def __click(self, pin: int, delay: float) -> bool:
        if not self.__lock.locked():
            async with self.__lock:
                for flag in (True, False):
                    GPIO.output(pin, flag)
                    await asyncio.sleep(delay)
                    return True
        return False
