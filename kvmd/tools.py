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
import operator
import functools
import multiprocessing.queues
import queue
import shlex

from typing import Generator
from typing import TypeVar


# =====
def remap(value: int, in_min: int, in_max: int, out_min: int, out_max: int) -> int:
    return int((value - in_min) * (out_max - out_min) // (in_max - in_min) + out_min)


# =====
def cmdfmt(cmd: list[str]) -> str:
    return " ".join(map(shlex.quote, cmd))


def efmt(ex: Exception) -> str:
    return f"{type(ex).__name__}: {ex}"


# =====
def rget(dct: dict, *keys: str) -> dict:
    result = functools.reduce((lambda nxt, key: nxt.get(key, {})), keys, dct)
    if not isinstance(result, dict):
        raise TypeError(f"Not a dict as result: {result!r} from {dct!r} at {list(keys)}")
    return result


_DictKeyT = TypeVar("_DictKeyT")
_DictValueT = TypeVar("_DictValueT")


def sorted_kvs(dct: dict[_DictKeyT, _DictValueT]) -> list[tuple[_DictKeyT, _DictValueT]]:
    return sorted(dct.items(), key=operator.itemgetter(0))


def swapped_kvs(dct: dict[_DictKeyT, _DictValueT]) -> dict[_DictValueT, _DictKeyT]:
    return {value: key for (key, value) in dct.items()}


# =====
def clear_queue(q: (multiprocessing.queues.Queue | asyncio.Queue)) -> None:  # pylint: disable=invalid-name
    for _ in range(q.qsize()):
        try:
            q.get_nowait()
        except (queue.Empty, asyncio.QueueEmpty):
            break


# =====
def build_cmd(cmd: list[str], cmd_remove: list[str], cmd_append: list[str]) -> list[str]:
    assert len(cmd) >= 1, cmd
    return [
        cmd[0],  # Executable
        *filter((lambda item: item not in cmd_remove), cmd[1:]),
        *cmd_append,
    ]


# =====
def passwds_splitted(text: str) -> Generator[tuple[int, str]]:
    for (lineno, line) in enumerate(text.split("\n")):
        line = line.rstrip("\r")
        ls = line.strip()
        if len(ls) == 0 or ls.startswith("#"):
            continue
        yield (lineno, line)
