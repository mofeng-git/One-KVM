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
import types

from typing import Dict
from typing import Type
from typing import AsyncGenerator

from . import MsdOperationError
from . import BaseMsd


# =====
class MsdCliOnlyError(MsdOperationError):
    def __init__(self) -> None:
        super().__init__("Only CLI")


# =====
class Plugin(BaseMsd):
    def get_state(self) -> Dict:
        return {
            "enabled": False,
            "multi": False,
            "online": False,
            "busy": False,
            "uploading": False,
            "written": 0,
            "current": None,
            "storage": None,
            "cdrom": None,
            "connected": False,
        }

    async def poll_state(self) -> AsyncGenerator[Dict, None]:
        while True:
            yield self.get_state()
            await asyncio.sleep(60)

    async def reset(self) -> None:
        raise MsdCliOnlyError()

    # =====

    async def connect(self) -> Dict:
        raise MsdCliOnlyError()

    async def disconnect(self) -> Dict:
        raise MsdCliOnlyError()

    async def select(self, name: str, cdrom: bool) -> Dict:
        raise MsdCliOnlyError()

    async def remove(self, name: str) -> Dict:
        raise MsdCliOnlyError()

    async def __aenter__(self) -> BaseMsd:
        raise MsdCliOnlyError()

    async def write_image_info(self, name: str, complete: bool) -> None:
        raise MsdCliOnlyError()

    async def write_image_chunk(self, chunk: bytes) -> int:
        raise MsdCliOnlyError()

    async def __aexit__(
        self,
        _exc_type: Type[BaseException],
        _exc: BaseException,
        _tb: types.TracebackType,
    ) -> None:

        raise MsdCliOnlyError()
