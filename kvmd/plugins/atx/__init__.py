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


from typing import Dict
from typing import AsyncGenerator
from typing import Type

from ... import aioregion

from .. import BasePlugin
from .. import get_plugin_class


# =====
class AtxError(Exception):
    pass


class AtxOperationError(AtxError):
    pass


class AtxIsBusyError(AtxOperationError, aioregion.RegionIsBusyError):
    pass


# =====
class BaseAtx(BasePlugin):
    def get_state(self) -> Dict:
        raise NotImplementedError

    async def poll_state(self) -> AsyncGenerator[Dict, None]:
        yield {}
        raise NotImplementedError

    async def power_on(self) -> bool:
        raise NotImplementedError

    async def power_off(self) -> bool:
        raise NotImplementedError

    async def power_off_hard(self) -> bool:
        raise NotImplementedError

    async def power_reset_hard(self) -> bool:
        raise NotImplementedError

    async def click_power(self) -> None:
        raise NotImplementedError

    async def click_power_long(self) -> None:
        raise NotImplementedError

    async def click_reset(self) -> None:
        raise NotImplementedError

    async def cleanup(self) -> None:
        pass


# =====
def get_atx_class(name: str) -> Type[BaseAtx]:
    return get_plugin_class("atx", (name or "none"))  # type: ignore
