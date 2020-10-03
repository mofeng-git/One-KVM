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

from typing import List
from typing import Dict
from typing import Callable
from typing import AsyncGenerator
from typing import TypeVar
from typing import Optional

import aiofiles

from ....logging import get_logger

from .... import env
from .... import aioproc

from .base import BaseInfoSubmanager


# =====
_RetvalT = TypeVar("_RetvalT")


# =====
class HwInfoSubmanager(BaseInfoSubmanager):
    def __init__(
        self,
        vcgencmd_cmd: List[str],
        state_poll: float,
    ) -> None:

        self.__vcgencmd_cmd = vcgencmd_cmd
        self.__state_poll = state_poll

    async def get_state(self) -> Dict:
        (model, cpu_temp, gpu_temp, throttling) = await asyncio.gather(
            self.__get_dt_model(),
            self.__get_cpu_temp(),
            self.__get_gpu_temp(),
            self.__get_throttling(),
        )
        return {
            "platform": {
                "type": "rpi",
                "base": model,
            },
            "health": {
                "temp": {
                    "cpu": cpu_temp,
                    "gpu": gpu_temp,
                },
                "throttling": throttling,
            },
        }

    async def poll_state(self) -> AsyncGenerator[Dict, None]:
        prev_state: Dict = {}
        while True:
            state = await self.get_state()
            if state != prev_state:
                yield state
                prev_state = state
            await asyncio.sleep(self.__state_poll)

    # =====

    async def __get_dt_model(self) -> Optional[str]:
        model_path = f"{env.PROCFS_PREFIX}/proc/device-tree/model"
        try:
            async with aiofiles.open(model_path) as model_file:
                return (await model_file.read()).strip(" \t\r\n\0")
        except Exception as err:
            get_logger(0).error("Can't read DT model from %s: %s", model_path, err)
            return None

    async def __get_cpu_temp(self) -> Optional[float]:
        temp_path = f"{env.SYSFS_PREFIX}/sys/class/thermal/thermal_zone0/temp"
        try:
            async with aiofiles.open(temp_path) as temp_file:
                return int((await temp_file.read()).strip()) / 1000
        except Exception as err:
            get_logger(0).error("Can't read CPU temp from %s: %s", temp_path, err)
            return None

    async def __get_throttling(self) -> Optional[Dict]:
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
            }
        return None

    async def __get_gpu_temp(self) -> Optional[float]:
        return (await self.__parse_vcgencmd(
            arg="measure_temp",
            parser=(lambda text: float(text.split("=")[1].split("'")[0])),
        ))

    async def __parse_vcgencmd(self, arg: str, parser: Callable[[str], _RetvalT]) -> Optional[_RetvalT]:
        cmd = [*self.__vcgencmd_cmd, arg]
        try:
            text = (await aioproc.read_process(cmd, err_to_null=True))[1]
        except Exception:
            get_logger(0).exception("Error while executing %s", cmd)
            return None
        try:
            return parser(text)
        except Exception as err:
            get_logger(0).error("Can't parse %s output: %r: %s", cmd, text, err)
            return None
