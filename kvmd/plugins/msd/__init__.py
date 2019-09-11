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


import types

from typing import Dict
from typing import Type
from typing import AsyncGenerator

from .. import BasePlugin
from .. import get_plugin_class


# =====
class MsdError(Exception):
    pass


class MsdOperationError(MsdError):
    pass


class MsdOfflineError(MsdOperationError):
    def __init__(self) -> None:
        super().__init__("MSD is not found")


class MsdAlreadyOnServerError(MsdOperationError):
    def __init__(self) -> None:
        super().__init__("MSD is already connected to Server")


class MsdAlreadyOnKvmError(MsdOperationError):
    def __init__(self) -> None:
        super().__init__("MSD is already connected to KVM")


class MsdNotOnKvmError(MsdOperationError):
    def __init__(self) -> None:
        super().__init__("MSD is not connected to KVM")


class MsdIsBusyError(MsdOperationError):
    def __init__(self) -> None:
        super().__init__("Performing another MSD operation, please try again later")


# =====
class BaseMsd(BasePlugin):
    def get_state(self) -> Dict:
        raise NotImplementedError

    async def poll_state(self) -> AsyncGenerator[Dict, None]:
        yield {}
        raise NotImplementedError

    async def cleanup(self) -> None:
        pass

    async def connect_to_kvm(self) -> Dict:
        raise NotImplementedError

    async def connect_to_server(self) -> Dict:
        raise NotImplementedError

    async def reset(self) -> None:
        raise NotImplementedError

    async def __aenter__(self) -> "BaseMsd":
        raise NotImplementedError

    def get_chunk_size(self) -> int:
        raise NotImplementedError

    async def write_image_info(self, name: str, complete: bool) -> None:
        raise NotImplementedError

    async def write_image_chunk(self, chunk: bytes) -> int:
        raise NotImplementedError

    async def __aexit__(
        self,
        _exc_type: Type[BaseException],
        _exc: BaseException,
        _tb: types.TracebackType,
    ) -> None:

        raise NotImplementedError


# =====
def get_msd_class(name: str) -> Type[BaseMsd]:
    return get_plugin_class("msd", (name or "disabled"))  # type: ignore
