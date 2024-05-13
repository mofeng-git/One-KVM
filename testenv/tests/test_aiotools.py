# ========================================================================== #
#                                                                            #
#    KVMD - The main PiKVM daemon.                                           #
#                                                                            #
#    Copyright (C) 2018-2024  Maxim Devaev <mdevaev@gmail.com>               #
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

import pytest

from kvmd.aiotools import AioExclusiveRegion
from kvmd.aiotools import shield_fg


# =====
class RegionIsBusyError(Exception):
    pass


# =====
@pytest.mark.asyncio
async def test_ok__region__access_one() -> None:
    region = AioExclusiveRegion(RegionIsBusyError)

    async def func() -> None:
        assert not region.is_busy()
        async with region:
            assert region.is_busy()
        assert not region.is_busy()

    await func()

    assert not region.is_busy()
    await region.exit()
    assert not region.is_busy()


@pytest.mark.asyncio
async def test_fail__region__access_one() -> None:
    region = AioExclusiveRegion(RegionIsBusyError)

    async def func() -> None:
        assert not region.is_busy()
        async with region:
            assert region.is_busy()
            await region.enter()
        assert not region.is_busy()

    with pytest.raises(RegionIsBusyError):
        await func()

    assert not region.is_busy()
    await region.exit()
    assert not region.is_busy()


# =====
@pytest.mark.asyncio
async def test_ok__region__access_two() -> None:
    region = AioExclusiveRegion(RegionIsBusyError)

    async def func1() -> None:
        async with region:
            await asyncio.sleep(1)
        print("done func1()")

    async def func2() -> None:
        await asyncio.sleep(2)
        print("waiking up func2()")
        async with region:
            await asyncio.sleep(1)
        print("done func2()")

    await asyncio.gather(func1(), func2())

    assert not region.is_busy()
    await region.exit()
    assert not region.is_busy()


@pytest.mark.asyncio
async def test_fail__region__access_two() -> None:
    region = AioExclusiveRegion(RegionIsBusyError)

    async def func1() -> None:
        async with region:
            await asyncio.sleep(2)
        print("done func1()")

    async def func2() -> None:
        await asyncio.sleep(1)
        async with region:
            await asyncio.sleep(1)
        print("done func2()")

    results = await asyncio.gather(func1(), func2(), return_exceptions=True)
    assert results[0] is None
    assert type(results[1]) is RegionIsBusyError  # pylint: disable=unidiomatic-typecheck

    assert not region.is_busy()
    await region.exit()
    assert not region.is_busy()


# =====
@pytest.mark.asyncio
async def test_ok__shield_fg() -> None:
    ops: list[str] = []

    async def foo(op: str, delay: float) -> None:  # pylint: disable=disallowed-name
        await asyncio.sleep(delay)
        ops.append(op)

    async def bar() -> None:  # pylint: disable=disallowed-name
        try:
            try:
                try:
                    raise RuntimeError()
                finally:
                    await shield_fg(foo("foo1", 2.0))
                    ops.append("foo1-noexc")
            finally:
                await shield_fg(foo("foo2", 1.0))
                ops.append("foo2-noexc")
        finally:
            ops.append("done")

    task = asyncio.create_task(bar())
    await asyncio.sleep(0.1)
    task.cancel()
    with pytest.raises(asyncio.CancelledError):
        await task
    assert ops == ["foo1", "foo2", "foo2-noexc", "done"]
