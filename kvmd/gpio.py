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
import contextlib

from typing import Tuple
from typing import List
from typing import Generator
from typing import Optional

from RPi import GPIO

from .logging import get_logger

from . import aiotools


# =====
@contextlib.contextmanager
def bcm() -> Generator[None, None, None]:
    logger = get_logger(2)
    GPIO.setmode(GPIO.BCM)
    logger.info("Configured GPIO mode as BCM")
    try:
        yield
    finally:
        GPIO.cleanup()
        logger.info("GPIO cleaned")


def set_output(pin: int, initial: bool=False) -> int:
    assert pin >= 0, pin
    GPIO.setup(pin, GPIO.OUT, initial=initial)
    return pin


def set_input(pin: int) -> int:
    assert pin >= 0, pin
    GPIO.setup(pin, GPIO.IN)
    return pin


def read(pin: int) -> bool:
    assert pin >= 0, pin
    return bool(GPIO.input(pin))


def write(pin: int, state: bool) -> None:
    assert pin >= 0, pin
    GPIO.output(pin, state)


class BatchReader:
    def __init__(self, pins: List[int], interval: float, notifier: aiotools.AioNotifier) -> None:
        self.__pins = pins
        self.__flags: Tuple[Optional[bool], ...] = (None,) * len(pins)
        self.__state = dict.fromkeys(pins, False)

        self.__interval = interval
        self.__notifier = notifier

    def get(self, pin: int) -> bool:
        return self.__state[pin]

    async def poll(self) -> None:
        if not self.__pins:
            await aiotools.wait_infinite()
        else:
            while True:
                flags = tuple(map(read, self.__pins))
                if flags != self.__flags:
                    self.__flags = flags
                    self.__state = dict(zip(self.__pins, flags))
                    await self.__notifier.notify()
                await asyncio.sleep(self.__interval)
