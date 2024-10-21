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


from typing import AsyncGenerator

from .... import aiotools

from .base import BaseInfoSubmanager


# =====
class AuthInfoSubmanager(BaseInfoSubmanager):
    def __init__(self, enabled: bool) -> None:
        self.__enabled = enabled
        self.__notifier = aiotools.AioNotifier()

    async def get_state(self) -> dict:
        return {"enabled": self.__enabled}

    async def trigger_state(self) -> None:
        self.__notifier.notify()

    async def poll_state(self) -> AsyncGenerator[(dict | None), None]:
        while True:
            await self.__notifier.wait()
            yield (await self.get_state())
