# ========================================================================== #
#                                                                            #
#    KVMD - The main PiKVM daemon.                                           #
#                                                                            #
#    Copyright (C) 2018-2023  Maxim Devaev <mdevaev@gmail.com>               #
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
import dataclasses

import gpiod

from . import aiotools


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
        pins: dict[int, AioReaderPinParams],
        notifier: aiotools.AioNotifier,
    ) -> None:

        self.__path = path
        self.__consumer = consumer
        self.__pins = dict(pins)
        self.__notifier = notifier

        self.__values: (dict[int, _DebouncedValue] | None) = None

        self.__thread = threading.Thread(target=self.__run, daemon=True)
        self.__stop_event = threading.Event()

        self.__loop: (asyncio.AbstractEventLoop | None) = None

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

        pins = sorted(self.__pins)
        with gpiod.request_lines(
            self.__path,
            consumer=self.__consumer,
            config={tuple(pins): gpiod.LineSettings(edge_detection=gpiod.line.Edge.BOTH)},
        ) as line_request:

            line_request.wait_edge_events(0.1)
            self.__values = {
                pin: _DebouncedValue(
                    initial=bool(value.value),
                    debounce=self.__pins[pin].debounce,
                    notifier=self.__notifier,
                    loop=self.__loop,
                )
                for (pin, value) in zip(pins, line_request.get_values(pins))
            }
            self.__loop.call_soon_threadsafe(self.__notifier.notify)

            while not self.__stop_event.is_set():
                if line_request.wait_edge_events(1):
                    new: dict[int, bool] = {}
                    for event in line_request.read_edge_events():
                        (pin, value) = self.__parse_event(event)
                        new[pin] = value
                    for (pin, value) in new.items():
                        self.__values[pin].set(value)
                else:  # Timeout
                    # XXX: Лимит был актуален для 1.6. Надо проверить, поменялось ли это в 2.x.
                    # Размер буфера ядра - 16 эвентов на линии. При превышении этого числа,
                    # новые эвенты потеряются. Это не баг, это фича, как мне объяснили в LKML.
                    # Штош. Будем с этим жить и синхронизировать состояния при таймауте.
                    for (pin, value) in zip(pins, line_request.get_values(pins)):
                        self.__values[pin].set(bool(value.value))  # type: ignore

    def __parse_event(self, event: gpiod.EdgeEvent) -> tuple[int, bool]:
        if event.event_type == event.Type.RISING_EDGE:
            return (event.line_offset, True)
        elif event.event_type == event.Type.FALLING_EDGE:
            return (event.line_offset, False)
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

        self.__queue: "asyncio.Queue[bool]" = asyncio.Queue()  # type: ignore
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
                self.__notifier.notify()
            await asyncio.sleep(self.__debounce)
