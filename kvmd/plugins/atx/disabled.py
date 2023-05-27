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


from typing import AsyncGenerator

from ... import aiotools

from . import AtxOperationError
from . import BaseAtx


# =====
class AtxDisabledError(AtxOperationError):
    def __init__(self) -> None:
        super().__init__("ATX is disabled")


# =====
class Plugin(BaseAtx):
    async def get_state(self) -> dict:
        return {
            "enabled": False,
            "busy": False,
            "leds": {
                "power": False,
                "hdd": False,
            },
        }

    async def poll_state(self) -> AsyncGenerator[dict, None]:
        while True:
            yield (await self.get_state())
            await aiotools.wait_infinite()

    # =====

    async def __stub(self, wait: bool) -> None:
        raise AtxDisabledError()

    power_on = power_off = power_off_hard = power_reset_hard = __stub
    click_power = click_power_long = click_reset = __stub
