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

from ...logging import get_logger

from ... import tools
from ... import aiotools
from ... import aioproc

from ...yamlconf import Option

from ...validators.os import valid_command

from . import GpioDriverOfflineError
from . import UserGpioModes
from . import BaseUserGpioDriver


# =====
class Plugin(BaseUserGpioDriver):  # pylint: disable=too-many-instance-attributes
    def __init__(  # pylint: disable=super-init-not-called
        self,
        instance_name: str,
        notifier: aiotools.AioNotifier,

        cmd: list[str],
    ) -> None:

        super().__init__(instance_name, notifier)

        self.__cmd = cmd

    @classmethod
    def get_plugin_options(cls) -> dict:
        return {
            "cmd": Option([], type=valid_command),
        }

    @classmethod
    def get_modes(cls) -> set[str]:
        return set([UserGpioModes.OUTPUT])

    @classmethod
    def get_pin_validator(cls) -> Callable[[Any], Any]:
        return str

    async def read(self, pin: str) -> bool:
        _ = pin
        return False

    async def write(self, pin: str, state: bool) -> None:
        _ = pin
        if not state:
            return
        try:
            proc = await aioproc.log_process(self.__cmd, logger=get_logger(0), prefix=str(self))
            if proc.returncode != 0:
                raise RuntimeError(f"Custom command error: retcode={proc.returncode}")
        except Exception as err:
            get_logger(0).error("Can't run custom command [ %s ]: %s",
                                tools.cmdfmt(self.__cmd), tools.efmt(err))
            raise GpioDriverOfflineError(self)

    def __str__(self) -> str:
        return f"CMD({self._instance_name})"

    __repr__ = __str__
