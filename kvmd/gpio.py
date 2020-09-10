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

from typing import List
from typing import Tuple
from typing import Set
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


def set_output(pin: int, initial: Optional[bool]) -> int:
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
    def __init__(
        self,
        pins: Set[int],
        edge_detection: bool,
        interval: float,
        notifier: aiotools.AioNotifier,
    ) -> None:

        self.__pins = sorted(pins)
        self.__edge_detection = edge_detection
        self.__interval = interval
        self.__notifier = notifier

        self.__state = {pin: read(pin) for pin in self.__pins}

        self.__loop: Optional[asyncio.AbstractEventLoop] = None  # Only for edge detection

        self.__flags: Tuple[Optional[bool], ...] = (None,) * len(self.__pins)  # Only for busyloop

    def get(self, pin: int) -> bool:
        return self.__state[pin]

    async def poll(self) -> None:
        if self.__edge_detection:
            await self.__poll_edge()
        else:
            await self.__poll_busyloop()

    # =====

    async def __poll_edge(self) -> None:
        assert self.__loop is None
        self.__loop = asyncio.get_running_loop()
        watched: List[int] = []
        try:
            for pin in self.__pins:
                GPIO.add_event_detect(
                    pin, GPIO.BOTH,
                    callback=self.__poll_edge_callback,
                    bouncetime=int(self.__interval * 1000),
                )
                watched.append(pin)
            await self.__notifier.notify()
            await aiotools.wait_infinite()
        finally:
            for pin in watched:
                GPIO.remove_event_detect(pin)

    def __poll_edge_callback(self, pin: int) -> None:
        assert self.__loop
        self.__state[pin] = read(pin)
        self.__loop.call_soon_threadsafe(self.__notifier.notify_sync)

    # =====

    async def __poll_busyloop(self) -> None:
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
