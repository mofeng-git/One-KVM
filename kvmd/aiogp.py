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


import os
import asyncio
import threading

from typing import Tuple
from typing import Dict
from typing import Optional

import gpiod

from . import aiotools


# =====
# XXX: Do not use this variable for any purpose other than testing.
# It can be removed at any time.
DEVICE_PATH = os.getenv("KVMD_GPIO_DEVICE_PATH", "/dev/gpiochip0")


# =====
async def pulse(line: gpiod.Line, delay: float, final: float) -> None:
    try:
        line.set_value(1)
        await asyncio.sleep(delay)
    finally:
        line.set_value(0)
        await asyncio.sleep(final)


class AioPinsReader:  # pylint: disable=too-many-instance-attributes
    def __init__(
        self,
        path: str,
        consumer: str,
        pins: Dict[int, bool],  # (pin, inverted)
        notifier: aiotools.AioNotifier,
    ) -> None:

        self.__path = path
        self.__consumer = consumer
        self.__pins = pins
        self.__notifier = notifier

        self.__state = dict.fromkeys(pins, 0)

        self.__loop: Optional[asyncio.AbstractEventLoop] = None

        self.__thread = threading.Thread(target=self.__run, daemon=True)
        self.__stop_event = threading.Event()

    def get(self, pin: int) -> bool:
        return (bool(self.__state[pin]) ^ self.__pins[pin])

    async def poll(self) -> None:
        if not self.__pins:
            await aiotools.wait_infinite()
        else:
            assert self.__loop is None
            self.__loop = asyncio.get_running_loop()
            self.__thread.start()
            try:
                await aiotools.run_async(self.__thread.join)
            finally:
                self.__stop_event.set()
                await aiotools.run_async(self.__thread.join)

    def __run(self) -> None:
        with gpiod.Chip(self.__path) as chip:
            pins = sorted(self.__pins)
            lines = chip.get_lines(pins)
            lines.request(self.__consumer, gpiod.LINE_REQ_EV_BOTH_EDGES)

            def read_state() -> Dict[int, int]:
                return dict(zip(pins, lines.get_values()))

            lines.event_wait(nsec=1)
            self.__state = read_state()
            self.__notify()

            while not self.__stop_event.is_set():
                changed = False
                ev_lines = lines.event_wait(1)
                if ev_lines:
                    for ev_line in ev_lines:
                        events = ev_line.event_read_multiple()
                        if events:
                            (pin, value) = self.__parse_event(events[-1])
                            if self.__state[pin] != value:
                                self.__state[pin] = value
                                changed = True
                else:  # Timeout
                    # Ensure state to avoid driver bugs
                    state = read_state()
                    if self.__state != state:
                        self.__state = state
                        changed = True
                if changed:
                    self.__notify()

    def __parse_event(self, event: gpiod.LineEvent) -> Tuple[int, int]:
        pin = event.source.offset()
        if event.type == gpiod.LineEvent.RISING_EDGE:
            return (pin, 1)
        elif event.type == gpiod.LineEvent.FALLING_EDGE:
            return (pin, 0)
        raise RuntimeError(f"Invalid event {event} type: {event.type}")

    def __notify(self) -> None:
        assert self.__loop
        self.__loop.call_soon_threadsafe(self.__notifier.notify_sync)
