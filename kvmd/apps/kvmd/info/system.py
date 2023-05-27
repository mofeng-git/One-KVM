# ========================================================================== #
#                                                                            #
#    KVMD - The main PiKVM daemon.                                           #
#                                                                            #
#    Copyright (C) 2018-2023  Maxim Devaev <mdevaev@gmail.com>               #
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

from ....logging import get_logger

from .... import aioproc

from .... import __version__

from .base import BaseInfoSubmanager


# =====
class SystemInfoSubmanager(BaseInfoSubmanager):
    def __init__(self, streamer_cmd: list[str]) -> None:
        self.__streamer_cmd = streamer_cmd

    async def get_state(self) -> dict:
        streamer_info = await self.__get_streamer_info()
        uname_info = platform.uname()  # Uname using the internal cache
        return {
            "kvmd": {"version": __version__},
            "streamer": streamer_info,
            "kernel": {
                field: getattr(uname_info, field)
                for field in ["system", "release", "version", "machine"]
            },
        }

    # =====

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
