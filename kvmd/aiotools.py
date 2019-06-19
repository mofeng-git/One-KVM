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
import functools
import contextlib

import typing

from typing import List
from typing import Callable
from typing import Coroutine
from typing import Generator
from typing import AsyncGenerator
from typing import TypeVar
from typing import Any

from . import aioregion

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
@contextlib.contextmanager
def unregion_only_on_exception(region: aioregion.AioExclusiveRegion) -> Generator[None, None, None]:
    region.enter()
    try:
        yield
    except:  # noqa: E722
        region.exit()
        raise


@contextlib.asynccontextmanager
async def unlock_only_on_exception(lock: asyncio.Lock) -> AsyncGenerator[None, None]:
    await lock.acquire()
    try:
        yield
    except:  # noqa: E722
        lock.release()
        raise
