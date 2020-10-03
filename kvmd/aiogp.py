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
import asyncio.queues
import threading
import dataclasses

from typing import Tuple
from typing import Dict
from typing import Optional

import gpiod

from . import aiotools


# =====
async def pulse(line: gpiod.Line, delay: float, final: float) -> None:
    try:
        line.set_value(1)
        await asyncio.sleep(delay)
    finally:
        line.set_value(0)
        await asyncio.sleep(final)


# =====
@dataclasses.dataclass(frozen=True)
class AioReaderPinParams:
    inverted: bool
    debounce: float


class AioReader:  # pylint: disable=too-many-instance-attributes
    def __init__(
        self,
        path: str,
        consumer: str,
        pins: Dict[int, AioReaderPinParams],
        notifier: aiotools.AioNotifier,
    ) -> None:

        self.__path = path
        self.__consumer = consumer
        self.__pins = pins
        self.__notifier = notifier

        self.__values: Optional[Dict[int, _DebouncedValue]] = None

        self.__thread = threading.Thread(target=self.__run, daemon=True)
        self.__stop_event = threading.Event()

        self.__loop: Optional[asyncio.AbstractEventLoop] = None

    def get(self, pin: int) -> bool:
        value = (self.__values[pin].get() if self.__values is not None else False)
        return (value ^ self.__pins[pin].inverted)

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
        assert self.__values is None
        assert self.__loop
        with gpiod.Chip(self.__path) as chip:
            pins = sorted(self.__pins)
            lines = chip.get_lines(pins)
            lines.request(self.__consumer, gpiod.LINE_REQ_EV_BOTH_EDGES)

            lines.event_wait(nsec=1)
            self.__values = {
                pin: _DebouncedValue(
                    initial=bool(value),
                    debounce=self.__pins[pin].debounce,
                    notifier=self.__notifier,
                    loop=self.__loop,
                )
                for (pin, value) in zip(pins, lines.get_values())
            }
            self.__loop.call_soon_threadsafe(self.__notifier.notify_sync)

            while not self.__stop_event.is_set():
                ev_lines = lines.event_wait(1)
                if ev_lines:
                    for ev_line in ev_lines:
                        events = ev_line.event_read_multiple()
                        if events:
                            (pin, value) = self.__parse_event(events[-1])
                            self.__values[pin].set(bool(value))
                else:  # Timeout
                    # Размер буфера ядра - 16 эвентов на линии. При превышении этого числа,
                    # новые эвенты потеряются. Это не баг, это фича, как мне объяснили в LKML.
                    # Штош. Будем с этим жить и синхронизировать состояния при таймауте.
                    for (pin, value) in zip(pins, lines.get_values()):
                        self.__values[pin].set(bool(value))

    def __parse_event(self, event: gpiod.LineEvent) -> Tuple[int, int]:
        pin = event.source.offset()
        if event.type == gpiod.LineEvent.RISING_EDGE:
            return (pin, 1)
        elif event.type == gpiod.LineEvent.FALLING_EDGE:
            return (pin, 0)
        raise RuntimeError(f"Invalid event {event} type: {event.type}")


class _DebouncedValue:
    def __init__(
        self,
        initial: bool,
        debounce: float,
        notifier: aiotools.AioNotifier,
        loop: asyncio.AbstractEventLoop,
    ) -> None:

        self.__value = initial
        self.__debounce = debounce
        self.__notifier = notifier
        self.__loop = loop

        self.__queue: asyncio.queues.Queue = asyncio.Queue(loop=loop)
        self.__task = loop.create_task(self.__consumer_task_loop())

    def set(self, value: bool) -> None:
        if self.__loop.is_running():
            self.__check_alive()
            self.__loop.call_soon_threadsafe(self.__queue.put_nowait, value)

    def get(self) -> bool:
        return self.__value

    def __check_alive(self) -> None:
        if self.__task.done() and not self.__task.cancelled():
            raise RuntimeError("Dead debounce consumer")

    async def __consumer_task_loop(self) -> None:
        while True:
            value = await self.__queue.get()
            while not self.__queue.empty():
                value = await self.__queue.get()
            if self.__value != value:
                self.__value = value
                await self.__notifier.notify()
            await asyncio.sleep(self.__debounce)
