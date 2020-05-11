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


import contextlib

from typing import Dict
from typing import Type
from typing import AsyncGenerator
from typing import Optional

from ...errors import OperationError
from ...errors import IsBusyError

from .. import BasePlugin
from .. import get_plugin_class


# =====
class MsdError(Exception):
    pass


class MsdOperationError(OperationError, MsdError):
    pass


class MsdIsBusyError(IsBusyError, MsdError):
    def __init__(self) -> None:
        super().__init__("Performing another MSD operation, please try again later")


class MsdOfflineError(MsdOperationError):
    def __init__(self) -> None:
        super().__init__("MSD is not found")


class MsdConnectedError(MsdOperationError):
    def __init__(self) -> None:
        super().__init__("MSD is connected to Server, but shouldn't for this operation")


class MsdDisconnectedError(MsdOperationError):
    def __init__(self) -> None:
        super().__init__("MSD is disconnected from Server, but should be for this operation")


class MsdImageNotSelected(MsdOperationError):
    def __init__(self) -> None:
        super().__init__("The image is not selected")


class MsdUnknownImageError(MsdOperationError):
    def __init__(self) -> None:
        super().__init__("The image is not found in the storage")


class MsdImageExistsError(MsdOperationError):
    def __init__(self) -> None:
        super().__init__("This image is already exists")


class MsdMultiNotSupported(MsdOperationError):
    def __init__(self) -> None:
        super().__init__("This MSD does not support storing multiple images")


class MsdCdromNotSupported(MsdOperationError):
    def __init__(self) -> None:
        super().__init__("This MSD does not support CD-ROM emulation")


# =====
class BaseMsd(BasePlugin):
    async def get_state(self) -> Dict:
        raise NotImplementedError()

    async def poll_state(self) -> AsyncGenerator[Dict, None]:
        if self is not None:  # XXX: Vulture and pylint hack
            raise NotImplementedError()
        yield

    async def reset(self) -> None:
        raise NotImplementedError()

    async def cleanup(self) -> None:
        pass

    # =====

    async def set_params(self, name: Optional[str]=None, cdrom: Optional[bool]=None) -> None:
        raise NotImplementedError()

    async def connect(self) -> None:
        raise NotImplementedError()

    async def disconnect(self) -> None:
        raise NotImplementedError()

    @contextlib.asynccontextmanager
    async def write_image(self, name: str) -> AsyncGenerator[None, None]:  # pylint: disable=unused-argument
        if self is not None:  # XXX: Vulture and pylint hack
            raise NotImplementedError()
        yield

    async def write_image_chunk(self, chunk: bytes) -> int:
        raise NotImplementedError()

    async def remove(self, name: str) -> None:
        raise NotImplementedError()


# =====
def get_msd_class(name: str) -> Type[BaseMsd]:
    return get_plugin_class("msd", name)  # type: ignore
