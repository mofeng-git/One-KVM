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
import platform

from typing import AsyncGenerator

from ....logging import get_logger

from .... import env
from .... import aiotools
from .... import aioproc

from .... import __version__

from .base import BaseInfoSubmanager


# =====
class SystemInfoSubmanager(BaseInfoSubmanager):
    def __init__(
        self,
        platform_path: str,
        streamer_cmd: list[str],
    ) -> None:

        self.__platform_path = platform_path
        self.__streamer_cmd = streamer_cmd

        self.__dt_cache: dict[str, str] = {}
        self.__notifier = aiotools.AioNotifier()

    async def get_state(self) -> dict:
        (
            base,
            serial,
            pl,
            streamer_info,
        ) = await asyncio.gather(
            self.__read_dt_file("model", upper=False),
            self.__read_dt_file("serial-number", upper=True),
            self.__read_platform_file(),
            self.__get_streamer_info(),
        )
        uname_info = platform.uname()  # Uname using the internal cache
        return {
            "kvmd": {"version": __version__},
            "streamer": streamer_info,
            "kernel": {
                field: getattr(uname_info, field)
                for field in ["system", "release", "version", "machine"]
            },
            "platform": {
                "type": "rpi",
                "base": base,
                "serial": serial,
                **pl,  # type: ignore
            },
        }

    async def trigger_state(self) -> None:
        self.__notifier.notify()

    async def poll_state(self) -> AsyncGenerator[(dict | None), None]:
        while True:
            await self.__notifier.wait()
            yield (await self.get_state())

    # =====

    async def __read_dt_file(self, name: str, upper: bool) -> (str | None):
        if name not in self.__dt_cache:
            path = os.path.join(f"{env.PROCFS_PREFIX}/proc/device-tree", name)
            try:
                value = (await aiotools.read_file(path)).strip(" \t\r\n\0")
                self.__dt_cache[name] = (value.upper() if upper else value)
            except Exception as ex:
                get_logger(0).error("Can't read DT %s from %s: %s", name, path, ex)
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

    async def __get_streamer_info(self) -> dict:
        version = ""
        features: dict[str, bool] = {}
        try:
            path = self.__streamer_cmd[0]
            ((_, version), (_, features_text)) = await asyncio.gather(
                aioproc.read_process([path, "--version"], err_to_null=True),
                aioproc.read_process([path, "--features"], err_to_null=True),
            )
        except Exception:
            get_logger(0).exception("Can't get streamer info")
        else:
            try:
                for line in features_text.split("\n"):
                    (status, name) = map(str.strip, line.split(" "))
                    features[name] = (status == "+")
            except Exception:
                get_logger(0).exception("Can't parse streamer features")
        return {
            "app": os.path.basename(path),
            "version": version,
            "features": features,
        }
