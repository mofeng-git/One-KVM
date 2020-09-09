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


from typing import Type
from typing import Optional
from typing import Any

from ...errors import OperationError

from ... import aiotools

from .. import BasePlugin
from .. import get_plugin_class


# =====
class GpioError(Exception):
    pass


class GpioOperationError(OperationError, GpioError):
    pass


class GpioDriverOfflineError(GpioOperationError):
    def __init__(self, driver: "BaseUserGpioDriver") -> None:
        super().__init__(f"GPIO driver {driver} is offline")


# =====
class BaseUserGpioDriver(BasePlugin):
    def __init__(  # pylint: disable=super-init-not-called
        self,
        instance_name: str,
        notifier: aiotools.AioNotifier,
        **_: Any,
    ) -> None:

        self._instance_name = instance_name
        self._notifier = notifier

    def get_instance_id(self) -> str:
        return self._instance_name

    def register_input(self, pin: int) -> None:
        raise NotImplementedError

    def register_output(self, pin: int, initial: Optional[bool]) -> None:
        raise NotImplementedError

    def prepare(self) -> None:
        raise NotImplementedError

    async def run(self) -> None:
        raise NotImplementedError

    def cleanup(self) -> None:
        raise NotImplementedError

    def read(self, pin: int) -> bool:
        raise NotImplementedError

    def write(self, pin: int, state: bool) -> None:
        raise NotImplementedError


# =====
def get_ugpio_driver_class(name: str) -> Type[BaseUserGpioDriver]:
    return get_plugin_class("ugpio", name)  # type: ignore
