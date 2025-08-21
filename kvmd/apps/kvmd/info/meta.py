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


import socket

from typing import AsyncGenerator

from ....logging import get_logger

from ....yamlconf.loader import load_yaml_file

from .... import aiotools

from .base import BaseInfoSubmanager


# =====
class MetaInfoSubmanager(BaseInfoSubmanager):
    def __init__(self, meta_path: str) -> None:
        self.__meta_path = meta_path
        self.__notifier = aiotools.AioNotifier()

    async def get_state(self) -> (dict | None):
        try:
            meta = ((await aiotools.run_async(load_yaml_file, self.__meta_path)) or {})
            if meta["server"]["host"] == "@auto":
                meta["server"]["host"] = socket.getfqdn()
            return meta
        except Exception:
            get_logger(0).exception("Can't parse meta")
        return None

    async def trigger_state(self) -> None:
        self.__notifier.notify()

    async def poll_state(self) -> AsyncGenerator[(dict | None), None]:
        while True:
            await self.__notifier.wait()
            yield (await self.get_state())
