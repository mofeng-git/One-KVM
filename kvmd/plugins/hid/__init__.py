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


import asyncio
import time

from typing import Iterable
from typing import AsyncGenerator
from typing import Any

from ...yamlconf import Option

from ...validators.basic import valid_bool

from .. import BasePlugin
from .. import get_plugin_class


# =====
class BaseHid(BasePlugin):
    def __init__(self, jiggler_enabled: bool, jiggler_active: bool) -> None:
        self.__jiggler_enabled = jiggler_enabled
        self.__jiggler_active = jiggler_active
        self.__jiggler_absolute = True
        self.__activity_ts = 0

    @classmethod
    def _get_jiggler_options(cls) -> dict[str, Any]:
        return {
            "jiggler": {
                "enabled": Option(False, type=valid_bool, unpack_as="jiggler_enabled"),
                "active":  Option(False, type=valid_bool, unpack_as="jiggler_active"),
            },
        }

    # =====

    def sysprep(self) -> None:
        raise NotImplementedError

    async def get_state(self) -> dict:
        raise NotImplementedError

    async def poll_state(self) -> AsyncGenerator[dict, None]:
        yield {}
        raise NotImplementedError

    async def reset(self) -> None:
        raise NotImplementedError

    async def cleanup(self) -> None:
        pass

    # =====

    def send_key_events(self, keys: Iterable[tuple[str, bool]]) -> None:
        raise NotImplementedError

    def send_mouse_button_event(self, button: str, state: bool) -> None:
        raise NotImplementedError

    def send_mouse_move_event(self, to_x: int, to_y: int) -> None:
        _ = to_x
        _ = to_y

    def send_mouse_relative_event(self, delta_x: int, delta_y: int) -> None:
        _ = delta_x
        _ = delta_y

    def send_mouse_wheel_event(self, delta_x: int, delta_y: int) -> None:
        raise NotImplementedError

    def set_params(
        self,
        keyboard_output: (str | None)=None,
        mouse_output: (str | None)=None,
        jiggler: (bool | None)=None,
    ) -> None:

        raise NotImplementedError

    def set_connected(self, connected: bool) -> None:
        _ = connected

    def clear_events(self) -> None:
        raise NotImplementedError

    # =====

    async def systask(self) -> None:
        factor = 1
        while True:
            if self.__jiggler_active and (self.__activity_ts + 60 < int(time.monotonic())):
                for _ in range(5):
                    if self.__jiggler_absolute:
                        self.send_mouse_move_event(100 * factor, 100 * factor)
                    else:
                        self.send_mouse_relative_event(10 * factor, 10 * factor)
                    factor *= -1
                    await asyncio.sleep(0.1)
            await asyncio.sleep(1)

    def _bump_activity(self) -> None:
        self.__activity_ts = int(time.monotonic())

    def _set_jiggler_absolute(self, absolute: bool) -> None:
        self.__jiggler_absolute = absolute

    def _set_jiggler_active(self, active: bool) -> None:
        if self.__jiggler_enabled:
            self.__jiggler_active = active

    def _get_jiggler_state(self) -> dict:
        return {
            "jiggler": {
                "enabled": self.__jiggler_enabled,
                "active":  self.__jiggler_active,
            },
        }


# =====
def get_hid_class(name: str) -> type[BaseHid]:
    return get_plugin_class("hid", name)  # type: ignore
