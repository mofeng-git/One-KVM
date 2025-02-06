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
import copy

from typing import Callable
from typing import AsyncGenerator
from typing import TypeVar

import psutil

from ....logging import get_logger

from .... import env
from .... import tools
from .... import aiotools
from .... import aioproc

from .base import BaseInfoSubmanager


# =====
_RetvalT = TypeVar("_RetvalT")


# =====
class HealthInfoSubmanager(BaseInfoSubmanager):
    def __init__(
        self,
        vcgencmd_cmd: list[str],
        ignore_past: bool,
        state_poll: float,
    ) -> None:

        self.__vcgencmd_cmd = vcgencmd_cmd
        self.__ignore_past = ignore_past
        self.__state_poll = state_poll

        self.__notifier = aiotools.AioNotifier()

    async def get_state(self) -> dict:
        (
            throttling,
            cpu_percent,
            cpu_temp,
            mem,
        ) = await asyncio.gather(
            self.__get_throttling(),
            self.__get_cpu_percent(),
            self.__get_cpu_temp(),
            self.__get_mem(),
        )
        return {
            "temp": {
                "cpu": cpu_temp,
            },
            "cpu": {
                "percent": cpu_percent,
            },
            "mem": mem,
            "throttling": throttling,
        }

    async def trigger_state(self) -> None:
        self.__notifier.notify(1)

    async def poll_state(self) -> AsyncGenerator[dict, None]:
        prev: dict = {}
        while True:
            if (await self.__notifier.wait(timeout=self.__state_poll)) > 0:
                prev = {}
            new = await self.get_state()
            if new != prev:
                prev = copy.deepcopy(new)
                yield new

    # =====

    async def __get_cpu_temp(self) -> (float | None):
        temp_path = f"{env.SYSFS_PREFIX}/sys/class/thermal/thermal_zone0/temp"
        try:
            return int((await aiotools.read_file(temp_path)).strip()) / 1000
        except Exception as ex:
            get_logger(0).error("Can't read CPU temp from %s: %s", temp_path, ex)
            return None

    async def __get_cpu_percent(self) -> (float | None):
        try:
            st = psutil.cpu_times_percent()
            user = st.user - st.guest
            nice = st.nice - st.guest_nice
            idle_all = st.idle + st.iowait
            system_all = st.system + st.irq + st.softirq
            virtual = st.guest + st.guest_nice
            total = max(1, user + nice + system_all + idle_all + st.steal + virtual)
            return int(
                st.nice / total * 100
                + st.user / total * 100
                + system_all / total * 100
                + (st.steal + st.guest) / total * 100
            )
        except Exception as ex:
            get_logger(0).error("Can't get CPU percent: %s", ex)
            return None

    async def __get_mem(self) -> dict:
        try:
            st = psutil.virtual_memory()
            return {
                "percent": st.percent,
                "total": st.total,
                "available": st.available,
            }
        except Exception as ex:
            get_logger(0).error("Can't get memory info: %s", ex)
            return {
                "percent": None,
                "total": None,
                "available": None,
            }

    async def __get_throttling(self) -> (dict | None):
        # https://www.raspberrypi.org/forums/viewtopic.php?f=63&t=147781&start=50#p972790
        flags = await self.__parse_vcgencmd(
            arg="get_throttled",
            parser=(lambda text: int(text.split("=")[-1].strip(), 16)),
        )
        if flags is not None:
            return {
                "raw_flags": flags,
                "parsed_flags": {
                    "undervoltage": {
                        "now": bool(flags & (1 << 0)),
                        "past": bool(flags & (1 << 16)),
                    },
                    "freq_capped": {
                        "now": bool(flags & (1 << 1)),
                        "past": bool(flags & (1 << 17)),
                    },
                    "throttled": {
                        "now": bool(flags & (1 << 2)),
                        "past": bool(flags & (1 << 18)),
                    },
                },
                "ignore_past": self.__ignore_past,
            }
        return None

    async def __parse_vcgencmd(self, arg: str, parser: Callable[[str], _RetvalT]) -> (_RetvalT | None):
        cmd = [*self.__vcgencmd_cmd, arg]
        try:
            text = (await aioproc.read_process(cmd, err_to_null=True))[1]
        except Exception:
            get_logger(0).exception("Error while executing: %s", tools.cmdfmt(cmd))
            return None
        try:
            return parser(text)
        except Exception as ex:
            get_logger(0).error("Can't parse [ %s ] output: %r: %s", tools.cmdfmt(cmd), text, tools.efmt(ex))
            return None
