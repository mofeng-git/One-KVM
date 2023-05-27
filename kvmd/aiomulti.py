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


import multiprocessing
import queue

from typing import Type
from typing import TypeVar
from typing import Generic

from . import aiotools


# =====
_QueueItemT = TypeVar("_QueueItemT")


async def queue_get_last(  # pylint: disable=invalid-name
    q: "multiprocessing.Queue[_QueueItemT]",
    timeout: float,
) -> tuple[bool, (_QueueItemT | None)]:

    return (await aiotools.run_async(queue_get_last_sync, q, timeout))


def queue_get_last_sync(  # pylint: disable=invalid-name
    q: "multiprocessing.Queue[_QueueItemT]",
    timeout: float,
) -> tuple[bool, (_QueueItemT | None)]:

    try:
        item = q.get(timeout=timeout)
        while not q.empty():
            item = q.get()
        return (True, item)
    except queue.Empty:
        return (False, None)


# =====
class AioProcessNotifier:
    def __init__(self) -> None:
        self.__queue: "multiprocessing.Queue[None]" = multiprocessing.Queue()

    def notify(self) -> None:
        self.__queue.put_nowait(None)

    async def wait(self) -> None:
        while not (await queue_get_last(self.__queue, 0.1))[0]:
            pass


# =====
_SharedFlagT = TypeVar("_SharedFlagT", int, bool)


class AioSharedFlags(Generic[_SharedFlagT]):
    def __init__(
        self,
        initial: dict[str, _SharedFlagT],
        notifier: AioProcessNotifier,
        type: Type[_SharedFlagT]=bool,  # pylint: disable=redefined-builtin
    ) -> None:

        self.__notifier = notifier
        self.__type: Type[_SharedFlagT] = type

        self.__flags = {
            key: multiprocessing.RawValue("i", int(value))  # type: ignore
            for (key, value) in initial.items()
        }

        self.__lock = multiprocessing.Lock()

    def update(self, **kwargs: _SharedFlagT) -> None:
        changed = False
        with self.__lock:
            for (key, value) in kwargs.items():
                value = int(value)  # type: ignore
                if self.__flags[key].value != value:
                    self.__flags[key].value = value
                    changed = True
        if changed:
            self.__notifier.notify()

    async def get(self) -> dict[str, _SharedFlagT]:
        return (await aiotools.run_async(self.__inner_get))

    def __inner_get(self) -> dict[str, _SharedFlagT]:
        with self.__lock:
            return {
                key: self.__type(shared.value)
                for (key, shared) in self.__flags.items()
            }
