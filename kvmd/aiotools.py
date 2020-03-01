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
import asyncio.queues
import functools
import contextlib
import types

import typing

from typing import List
from typing import Callable
from typing import Coroutine
from typing import AsyncGenerator
from typing import Type
from typing import TypeVar
from typing import Any

import aiofiles
import aiofiles.base

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


def muted(msg: str) -> Callable[[_MethodT], Callable[..., None]]:
    def make_wrapper(method: _MethodT) -> Callable[..., None]:
        @functools.wraps(method)
        async def wrapper(*args: Any, **kwargs: Any) -> None:
            try:
                await method(*args, **kwargs)
            except asyncio.CancelledError:  # pylint: disable=try-except-raise
                raise
            except Exception:
                get_logger(0).exception(msg)
        return typing.cast(Callable[..., None], wrapper)
    return make_wrapper


def tasked(method: Callable[..., Any]) -> Callable[..., asyncio.Task]:
    @functools.wraps(method)
    async def wrapper(*args: Any, **kwargs: Any) -> asyncio.Task:
        return create_short_task(method(*args, **kwargs))
    return typing.cast(Callable[..., asyncio.Task], wrapper)


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


# =====
@contextlib.asynccontextmanager
async def unlock_only_on_exception(lock: asyncio.Lock) -> AsyncGenerator[None, None]:
    await lock.acquire()
    try:
        yield
    except:  # noqa: E722
        lock.release()
        raise


# =====
async def afile_write_now(afile: aiofiles.base.AiofilesContextManager, data: bytes) -> None:
    await afile.write(data)
    await afile.flush()
    await run_async(os.fsync, afile.fileno())


# =====
class AioExclusiveRegion:
    def __init__(self, exc_type: Type[Exception]) -> None:
        self.__exc_type = exc_type
        self.__busy = False

    def is_busy(self) -> bool:
        return self.__busy

    async def enter(self) -> None:
        if not self.__busy:
            self.__busy = True
            return
        raise self.__exc_type()

    async def exit(self) -> None:
        self.__busy = False

    @contextlib.asynccontextmanager
    async def exit_only_on_exception(self) -> AsyncGenerator[None, None]:
        await self.enter()
        try:
            yield
        except:  # noqa: E722
            await self.exit()
            raise

    async def __aenter__(self) -> None:
        await self.enter()

    async def __aexit__(
        self,
        _exc_type: Type[BaseException],
        _exc: BaseException,
        _tb: types.TracebackType,
    ) -> None:
        await self.exit()


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
