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
from typing import TypeVar
from typing import Any


# =====
_AtomicF = TypeVar("_AtomicF", bound=Callable[..., Any])


def atomic(method: _AtomicF) -> _AtomicF:
    @functools.wraps(method)
    async def wrapper(*args: object, **kwargs: object) -> Any:
        return (await asyncio.shield(method(*args, **kwargs)))
    return typing.cast(_AtomicF, wrapper)


def task(method: Callable[..., Any]) -> Callable[..., asyncio.Task]:
    @functools.wraps(method)
    async def wrapper(*args: object, **kwargs: object) -> asyncio.Task:
        return asyncio.create_task(method(*args, **kwargs))
    return typing.cast(Callable[..., asyncio.Task], wrapper)
