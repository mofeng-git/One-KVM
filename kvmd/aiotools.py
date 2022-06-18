# ========================================================================== #
#                                                                            #
#    KVMD - The main PiKVM daemon.                                           #
#                                                                            #
#    Copyright (C) 2018-2022  Maxim Devaev <mdevaev@gmail.com>               #
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
import signal
import asyncio
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
_FunctionT = TypeVar("_FunctionT", bound=Callable[..., Any])
_RetvalT = TypeVar("_RetvalT")


# =====
def atomic(func: _FunctionT) -> _FunctionT:
    @functools.wraps(func)
    async def wrapper(*args: Any, **kwargs: Any) -> Any:
        return (await asyncio.shield(func(*args, **kwargs)))
    return typing.cast(_FunctionT, wrapper)


# =====
_ATTR_SHORT_TASK = "_aiotools_short_task"


def create_short_task(coro: Coroutine) -> asyncio.Task:
    task = asyncio.create_task(coro)
    setattr(task, _ATTR_SHORT_TASK, True)
    return task


async def wait_all_short_tasks() -> None:
    await asyncio.gather(*[
        task
        for task in asyncio.all_tasks()
        if getattr(task, _ATTR_SHORT_TASK, False)
    ], return_exceptions=True)


# =====
_ATTR_DEADLY_TASK = "_aiotools_deadly_task"


def create_deadly_task(name: str, coro: Coroutine) -> asyncio.Task:
    logger = get_logger()

    async def wrapper() -> None:
        try:
            await coro
            raise RuntimeError(f"Deadly task is dead: {name}")
        except asyncio.CancelledError:
            pass
        except Exception:
            logger.exception("Unhandled exception in deadly task, killing myself ...")
            pid = os.getpid()
            if pid == 1:
                os._exit(1)  # Docker workaround  # pylint: disable=protected-access
            else:
                os.kill(pid, signal.SIGTERM)

    task = asyncio.create_task(wrapper())
    setattr(task, _ATTR_DEADLY_TASK, True)
    return task


async def stop_all_deadly_tasks() -> None:
    tasks: List[asyncio.Task] = []
    for task in asyncio.all_tasks():
        if getattr(task, _ATTR_DEADLY_TASK, False):
            tasks.append(task)
            task.cancel()
    await asyncio.gather(*tasks, return_exceptions=True)


# =====
async def run_async(func: Callable[..., _RetvalT], *args: Any) -> _RetvalT:
    return (await asyncio.get_running_loop().run_in_executor(None, func, *args))


def run_sync(coro: Coroutine[Any, Any, _RetvalT]) -> _RetvalT:
    return asyncio.get_event_loop().run_until_complete(coro)


# =====
async def wait_infinite() -> None:
    while True:
        await asyncio.sleep(3600)


async def wait_first(*aws: Awaitable) -> Tuple[Set[asyncio.Task], Set[asyncio.Task]]:
    return (await asyncio.wait(list(aws), return_when=asyncio.FIRST_COMPLETED))


# =====
async def close_writer(writer: asyncio.StreamWriter) -> bool:
    closing = writer.is_closing()
    if not closing:
        writer.transport.abort()  # type: ignore
        writer.close()
    try:
        await writer.wait_closed()
    except Exception:
        pass
    return (not closing)


# =====
def run(coro: Coroutine) -> None:
    def sigint_handler() -> None:
        raise KeyboardInterrupt()

    def sigterm_handler() -> None:
        raise SystemExit()

    loop = asyncio.get_event_loop()
    loop.add_signal_handler(signal.SIGINT, sigint_handler)
    loop.add_signal_handler(signal.SIGTERM, sigterm_handler)
    loop.run_until_complete(coro)


# =====
class AioNotifier:
    def __init__(self) -> None:
        self.__queue: "asyncio.Queue[None]" = asyncio.Queue()

    async def notify(self) -> None:
        await self.__queue.put(None)

    def notify_sync(self) -> None:
        self.__queue.put_nowait(None)

    async def wait(self, timeout: Optional[float]=None) -> None:
        if timeout is None:
            await self.__queue.get()
        else:
            try:
                await asyncio.wait_for(self.__queue.get(), timeout=timeout)
            except asyncio.TimeoutError:
                return  # False
        while not self.__queue.empty():
            await self.__queue.get()
        # return True


# =====
class AioStage:
    def __init__(self) -> None:
        self.__fut = asyncio.Future()  # type: ignore

    def set_passed(self, multi: bool=False) -> None:
        if multi and self.__fut.done():
            return
        self.__fut.set_result(None)

    def is_passed(self) -> bool:
        return self.__fut.done()

    async def wait_passed(self, timeout: float=-1) -> bool:
        if timeout >= 0:
            try:
                await asyncio.wait_for(self.__fut, timeout=timeout)
            except asyncio.TimeoutError:
                return False
        else:
            await self.__fut
        return True


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
    func: Callable[..., Coroutine[Any, Any, None]],
    *args: Any,
    **kwargs: Any,
) -> None:

    entered = asyncio.Future()  # type: ignore

    async def wrapper() -> None:
        try:
            async with region:
                entered.set_result(None)
                await func(*args, **kwargs)
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
