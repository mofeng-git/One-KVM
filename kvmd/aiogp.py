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
import threading

from typing import List
from typing import Optional

import gpiod

from . import aiotools


# =====
class AioPinsReader(threading.Thread):
    def __init__(
        self,
        path: str,
        consumer: str,
        pins: List[int],
        inverted: List[bool],
        notifier: aiotools.AioNotifier,
    ) -> None:

        assert len(pins) == len(inverted)
        super().__init__(daemon=True)

        self.__path = path
        self.__consumer = consumer
        self.__pins = pins
        self.__inverted = dict(zip(pins, inverted))
        self.__notifier = notifier

        self.__state = dict.fromkeys(pins, False)

        self.__stop_event = threading.Event()
        self.__loop: Optional[asyncio.AbstractEventLoop] = None

    def get(self, pin: int) -> bool:
        return (self.__state[pin] ^ self.__inverted[pin])

    async def poll(self) -> None:
        if not self.__pins:
            await aiotools.wait_infinite()
        else:
            assert self.__loop is None
            self.__loop = asyncio.get_running_loop()
            self.start()
            try:
                await aiotools.run_async(self.join)
            finally:
                self.__stop_event.set()
                await aiotools.run_async(self.join)

    def run(self) -> None:
        assert self.__loop
        with gpiod.Chip(self.__path) as chip:
            lines = chip.get_lines(self.__pins)
            lines.request(self.__consumer, gpiod.LINE_REQ_EV_BOTH_EDGES)

            lines.event_wait(nsec=1)
            self.__state = {
                pin: bool(value)
                for (pin, value) in zip(self.__pins, lines.get_values())
            }
            self.__loop.call_soon_threadsafe(self.__notifier.notify_sync)

            while not self.__stop_event.is_set():
                ev_lines = lines.event_wait(10)
                if ev_lines:
                    for ev_lines in lines.event_wait(1):
                        for ev_line in ev_lines:
                            event = ev_line.event_read()
                            if event.type == gpiod.LineEvent.RISING_EDGE:
                                value = True
                            elif event.type == gpiod.LineEvent.FALLING_EDGE:
                                value = False
                            else:
                                raise RuntimeError(f"Invalid event {event} type: {event.type}")
                            self.__state[event.source.offset()] = value
                        self.__loop.call_soon_threadsafe(self.__notifier.notify_sync)
