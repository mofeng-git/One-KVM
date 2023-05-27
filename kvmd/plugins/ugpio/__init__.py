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


from typing import Callable
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
class UserGpioModes:
    INPUT = "input"
    OUTPUT = "output"
    ALL = set([INPUT, OUTPUT])


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

    @classmethod
    def get_modes(cls) -> set[str]:
        return set(UserGpioModes.ALL)

    @classmethod
    def get_pin_validator(cls) -> Callable[[Any], Any]:
        # XXX: The returned value will be forcibly converted to a string
        # in kvmd/apps/kvmd/ugpio.py, i.e. AFTER validation.
        raise NotImplementedError

    def register_input(self, pin: str, debounce: float) -> None:
        _ = pin
        _ = debounce

    def register_output(self, pin: str, initial: (bool | None)) -> None:
        _ = pin
        _ = initial

    def prepare(self) -> None:
        pass

    async def run(self) -> None:
        await aiotools.wait_infinite()

    async def cleanup(self) -> None:
        pass

    async def read(self, pin: str) -> bool:
        raise NotImplementedError

    async def write(self, pin: str, state: bool) -> None:
        raise NotImplementedError


# =====
def get_ugpio_driver_class(name: str) -> type[BaseUserGpioDriver]:
    return get_plugin_class("ugpio", name)  # type: ignore
