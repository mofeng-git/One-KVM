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
class MsdDisabledError(MsdOperationError):
    def __init__(self) -> None:
        super().__init__("MSD is disabled")


# =====
class Plugin(BaseMsd):
    def get_state(self) -> Dict:
        return {
            "enabled": False,
            "online": False,
            "busy": False,
            "uploading": False,
            "written": False,
            "info": None,
            "connected_to": None,
        }

    async def poll_state(self) -> AsyncGenerator[Dict, None]:
        while True:
            yield self.get_state()
            await asyncio.sleep(60)

    async def connect_to_kvm(self) -> Dict:
        raise MsdDisabledError()

    async def connect_to_server(self) -> Dict:
        raise MsdDisabledError()

    async def reset(self) -> None:
        raise MsdDisabledError()

    async def __aenter__(self) -> BaseMsd:
        raise MsdDisabledError()

    def get_chunk_size(self) -> int:
        raise MsdDisabledError()

    async def write_image_info(self, name: str, complete: bool) -> None:
        raise MsdDisabledError()

    async def write_image_chunk(self, chunk: bytes) -> int:
        raise MsdDisabledError()

    async def __aexit__(
        self,
        _exc_type: Type[BaseException],
        _exc: BaseException,
        _tb: types.TracebackType,
    ) -> None:

        raise MsdDisabledError()
