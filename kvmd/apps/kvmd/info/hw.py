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


import os
import asyncio

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
class HwInfoSubmanager(BaseInfoSubmanager):
    def __init__(
        self,
        platform_path: str,
        vcgencmd_cmd: list[str],
        ignore_past: bool,
        state_poll: float,
    ) -> None:

        self.__platform_path = platform_path
        self.__vcgencmd_cmd = vcgencmd_cmd
        self.__ignore_past = ignore_past
        self.__state_poll = state_poll

        self.__dt_cache: dict[str, str] = {}

    async def get_state(self) -> dict:
        (
            base,
            serial,
            platform,
            throttling,
            cpu_percent,
            cpu_temp,
            mem,
        ) = await asyncio.gather(
            self.__read_dt_file("model"),
            self.__read_dt_file("serial-number"),
            self.__read_platform_file(),
            self.__get_throttling(),
            self.__get_cpu_percent(),
            self.__get_cpu_temp(),
            self.__get_mem(),
        )
        return {
            "platform": {
                "type": "rpi",
                "base": base,
                "serial": serial,
                **platform,  # type: ignore
            },
            "health": {
                "temp": {
                    "cpu": cpu_temp,
                },
                "cpu": {
                    "percent": cpu_percent,
                },
                "mem": mem,
                "throttling": throttling,
            },
        }

    async def poll_state(self) -> AsyncGenerator[dict, None]:
        prev_state: dict = {}
        while True:
            state = await self.get_state()
            if state != prev_state:
                yield state
                prev_state = state
            await asyncio.sleep(self.__state_poll)

    # =====

    async def __read_dt_file(self, name: str) -> (str | None):
        if name not in self.__dt_cache:
            path = os.path.join(f"{env.PROCFS_PREFIX}/proc/device-tree", name)
            try:
                self.__dt_cache[name] = (await aiotools.read_file(path)).strip(" \t\r\n\0")
            except Exception as err:
                get_logger(0).error("Can't read DT %s from %s: %s", name, path, err)
                return None
        return self.__dt_cache[name]

    async def __read_platform_file(self) -> dict:
        try:
            text = await aiotools.read_file(self.__platform_path)
            parsed: dict[str, str] = {}
            for row in text.split("\n"):
                row = row.strip()
                if row:
                    (key, value) = row.split("=", 1)
                    parsed[key.strip()] = value.strip()
            return {
                "model": parsed["PIKVM_MODEL"],
                "video": parsed["PIKVM_VIDEO"],
                "board": parsed["PIKVM_BOARD"],
            }
        except Exception:
            get_logger(0).exception("Can't read device model")
            return {"model": None, "video": None, "board": None}

    async def __get_cpu_temp(self) -> (float | None):
        temp_path = f"{env.SYSFS_PREFIX}/sys/class/thermal/thermal_zone0/temp"
        try:
            return int((await aiotools.read_file(temp_path)).strip()) / 1000
        except Exception as err:
            get_logger(0).error("Can't read CPU temp from %s: %s", temp_path, err)
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
        except Exception as err:
            get_logger(0).error("Can't get CPU percent: %s", err)
            return None

    async def __get_mem(self) -> dict:
        try:
            st = psutil.virtual_memory()
            return {
                "percent": st.percent,
                "total": st.total,
                "available": st.available,
            }
        except Exception as err:
            get_logger(0).error("Can't get memory info: %s", err)
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
        except Exception as err:
            get_logger(0).error("Can't parse [ %s ] output: %r: %s", tools.cmdfmt(cmd), text, tools.efmt(err))
            return None
