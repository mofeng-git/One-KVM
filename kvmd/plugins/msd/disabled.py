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


import contextlib

from typing import AsyncGenerator

from ... import aiotools

from . import MsdOperationError
from . import BaseMsdReader
from . import BaseMsdWriter
from . import BaseMsd


# =====
class MsdDisabledError(MsdOperationError):
    def __init__(self) -> None:
        super().__init__("MSD is disabled")


# =====
class Plugin(BaseMsd):
    async def get_state(self) -> dict:
        return {
            "enabled": False,
            "online": False,
            "busy": False,
            "storage": None,
            "drive": None,
        }

    async def poll_state(self) -> AsyncGenerator[dict, None]:
        while True:
            yield (await self.get_state())
            await aiotools.wait_infinite()

    async def reset(self) -> None:
        raise MsdDisabledError()

    # =====

    async def set_params(
        self,
        name: (str | None)=None,
        cdrom: (bool | None)=None,
        rw: (bool | None)=None,
    ) -> None:

        raise MsdDisabledError()

    async def set_connected(self, connected: bool) -> None:
        raise MsdDisabledError()

    @contextlib.asynccontextmanager
    async def read_image(self, name: str) -> AsyncGenerator[BaseMsdReader, None]:
        if self is not None:  # XXX: Vulture and pylint hack
            raise MsdDisabledError()
        yield BaseMsdReader()

    @contextlib.asynccontextmanager
    async def write_image(self, name: str, size: int, remove_incomplete: (bool | None)) -> AsyncGenerator[BaseMsdWriter, None]:
        if self is not None:  # XXX: Vulture and pylint hack
            raise MsdDisabledError()
        yield BaseMsdWriter()

    async def remove(self, name: str) -> None:
        raise MsdDisabledError()
