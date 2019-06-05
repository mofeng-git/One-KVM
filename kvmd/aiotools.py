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

import typing

from typing import Callable
from typing import Coroutine
from typing import TypeVar
from typing import Any


# =====
_AtomicF = TypeVar("_AtomicF", bound=Callable[..., Any])


def atomic(method: _AtomicF) -> _AtomicF:
    @functools.wraps(method)
    async def wrapper(*args: object, **kwargs: object) -> Any:
        return (await asyncio.shield(method(*args, **kwargs)))
    return typing.cast(_AtomicF, wrapper)


def tasked(method: Callable[..., Any]) -> Callable[..., asyncio.Task]:
    @functools.wraps(method)
    async def wrapper(*args: object, **kwargs: object) -> asyncio.Task:
        return create_short_task(method(*args, **kwargs))
    return typing.cast(Callable[..., asyncio.Task], wrapper)


_ATTR_SHORT_TASK = "_aiotools_short_task"


def create_short_task(coro: Coroutine) -> asyncio.Task:
    task = asyncio.create_task(coro)
    setattr(task, _ATTR_SHORT_TASK, True)
    return task


async def gather_short_tasks() -> None:
    await asyncio.gather(*[
        task
        for task in asyncio.Task.all_tasks()
        if getattr(task, _ATTR_SHORT_TASK, False)
    ])
