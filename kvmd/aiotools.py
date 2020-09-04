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
import functools
import types

import typing

from typing import Tuple
from typing import List
from typing import Set
from typing import Callable
from typing import Awaitable
from typing import Coroutine
from typing import Type
from typing import TypeVar
from typing import Optional
from typing import Any

from .logging import get_logger


# =====
_ATTR_SHORT_TASK = "_aiotools_short_task"

_MethodT = TypeVar("_MethodT", bound=Callable[..., Any])
_RetvalT = TypeVar("_RetvalT")


# =====
def atomic(method: _MethodT) -> _MethodT:
    @functools.wraps(method)
    async def wrapper(*args: Any, **kwargs: Any) -> Any:
        return (await asyncio.shield(method(*args, **kwargs)))
    return typing.cast(_MethodT, wrapper)


# =====
def create_short_task(coro: Coroutine) -> asyncio.Task:
    task = asyncio.create_task(coro)
    setattr(task, _ATTR_SHORT_TASK, True)
    return task


def get_short_tasks() -> List[asyncio.Task]:
    return [
        task
        for task in asyncio.Task.all_tasks()
        if getattr(task, _ATTR_SHORT_TASK, False)
    ]


# =====
async def run_async(method: Callable[..., _RetvalT], *args: Any) -> _RetvalT:
    return (await asyncio.get_running_loop().run_in_executor(None, method, *args))


def run_sync(coro: Coroutine[Any, Any, _RetvalT]) -> _RetvalT:
    return asyncio.get_event_loop().run_until_complete(coro)


# =====
async def wait_infinite() -> None:
    await asyncio.get_event_loop().create_future()


async def wait_first(*aws: Awaitable) -> Tuple[Set[asyncio.Future], Set[asyncio.Future]]:
    return (await asyncio.wait(list(aws), return_when=asyncio.FIRST_COMPLETED))


# =====
class AioNotifier:
    def __init__(self) -> None:
        self.__queue: asyncio.queues.Queue = asyncio.Queue()

    async def notify(self) -> None:
        await self.__queue.put(None)

    async def wait(self) -> None:
        await self.__queue.get()
        while not self.__queue.empty():
            await self.__queue.get()


# =====
class AioExclusiveRegion:
    def __init__(
        self,
        exc_type: Type[Exception],
        notifier: Optional[AioNotifier]=None,
    ) -> None:

        self.__exc_type = exc_type
        self.__notifier = notifier

        self.__busy = False

    def get_exc_type(self) -> Type[Exception]:
        return self.__exc_type

    def is_busy(self) -> bool:
        return self.__busy

    async def enter(self) -> None:
        if not self.__busy:
            self.__busy = True
            try:
                if self.__notifier:
                    await self.__notifier.notify()
            except:  # noqa: E722
                self.__busy = False
                raise
            return
        raise self.__exc_type()

    async def exit(self) -> None:
        self.__busy = False
        if self.__notifier:
            await self.__notifier.notify()

    async def __aenter__(self) -> None:
        await self.enter()

    async def __aexit__(
        self,
        _exc_type: Type[BaseException],
        _exc: BaseException,
        _tb: types.TracebackType,
    ) -> None:

        await self.exit()


async def run_region_task(
    msg: str,
    region: AioExclusiveRegion,
    method: Callable[..., Coroutine[Any, Any, None]],
    *args: Any,
    **kwargs: Any,
) -> None:

    entered = asyncio.Future()  # type: ignore

    async def wrapper() -> None:
        try:
            async with region:
                entered.set_result(None)
                await method(*args, **kwargs)
        except region.get_exc_type():
            raise
        except Exception:
            get_logger(0).exception(msg)

    task = create_short_task(wrapper())
    await asyncio.wait([entered, task], return_when=asyncio.FIRST_COMPLETED)

    if entered.done():
        return
    exc = task.exception()
    if exc is not None:
        raise exc
