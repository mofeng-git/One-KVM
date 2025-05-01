# ========================================================================== #
#                                                                            #
#    KVMD - The main PiKVM daemon.                                           #
#                                                                            #
#    Copyright (C) 2018-2024  Maxim Devaev <mdevaev@gmail.com>               #
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
import functools
import time

from typing import Iterable
from typing import Callable
from typing import AsyncGenerator
from typing import Any

from evdev import ecodes

from ...yamlconf import Option

from ...validators.basic import valid_bool
from ...validators.basic import valid_int_f1
from ...validators.basic import valid_string_list
from ...validators.hid import valid_hid_key
from ...validators.hid import valid_hid_mouse_move

from ...keyboard.mappings import WEB_TO_EVDEV
from ...keyboard.mappings import EvdevModifiers
from ...mouse import MouseRange

from .. import BasePlugin
from .. import get_plugin_class


# =====
class BaseHid(BasePlugin):  # pylint: disable=too-many-instance-attributes
    def __init__(
        self,
        ignore_keys: list[str],

        mouse_x_min: int,
        mouse_x_max: int,
        mouse_y_min: int,
        mouse_y_max: int,

        jiggler_enabled: bool,
        jiggler_active: bool,
        jiggler_interval: int,
    ) -> None:

        self.__ignore_keys = [WEB_TO_EVDEV[key] for key in ignore_keys]

        self.__mouse_x_range = (mouse_x_min, mouse_x_max)
        self.__mouse_y_range = (mouse_y_min, mouse_y_max)

        self.__j_enabled = jiggler_enabled
        self.__j_active = jiggler_active
        self.__j_interval = jiggler_interval
        self.__j_absolute = True
        self.__j_activity_ts = 0
        self.__j_last_x = 0
        self.__j_last_y = 0

    @classmethod
    def _get_base_options(cls) -> dict[str, Any]:
        return {
            "ignore_keys": Option([], type=functools.partial(valid_string_list, subval=valid_hid_key)),
            "mouse_x_range": {
                "min": Option(MouseRange.MIN, type=valid_hid_mouse_move, unpack_as="mouse_x_min"),
                "max": Option(MouseRange.MAX, type=valid_hid_mouse_move, unpack_as="mouse_x_max"),
            },
            "mouse_y_range": {
                "min": Option(MouseRange.MIN, type=valid_hid_mouse_move, unpack_as="mouse_y_min"),
                "max": Option(MouseRange.MAX, type=valid_hid_mouse_move, unpack_as="mouse_y_max"),
            },
            "jiggler": {
                "enabled":  Option(True,  type=valid_bool, unpack_as="jiggler_enabled"),
                "active":   Option(False, type=valid_bool, unpack_as="jiggler_active"),
                "interval": Option(60,    type=valid_int_f1, unpack_as="jiggler_interval"),
            },
        }

    # =====

    def sysprep(self) -> None:
        raise NotImplementedError

    async def get_state(self) -> dict:
        raise NotImplementedError

    async def trigger_state(self) -> None:
        raise NotImplementedError

    async def poll_state(self) -> AsyncGenerator[dict, None]:
        # ==== Granularity table ====
        #   - enabled   -- Full
        #   - online    -- Partial
        #   - busy      -- Partial
        #   - connected -- Partial, nullable
        #   - keyboard.online  -- Partial
        #   - keyboard.outputs -- Partial
        #   - keyboard.leds    -- Partial
        #   - mouse.online     -- Partial
        #   - mouse.outputs    -- Partial, follows with absolute
        #   - mouse.absolute   -- Partial, follows with outputs
        # ===========================

        yield {}
        raise NotImplementedError

    async def reset(self) -> None:
        raise NotImplementedError

    async def cleanup(self) -> None:
        pass

    def set_params(
        self,
        keyboard_output: (str | None)=None,
        mouse_output: (str | None)=None,
        jiggler: (bool | None)=None,
    ) -> None:

        raise NotImplementedError

    def set_connected(self, connected: bool) -> None:
        _ = connected

    # =====

    async def send_key_events(
        self,
        keys: Iterable[tuple[int, bool]],
        no_ignore_keys: bool=False,
        slow: bool=False,
    ) -> None:

        for (key, state) in keys:
            if no_ignore_keys or key not in self.__ignore_keys:
                if slow:
                    await asyncio.sleep(0.02)
                self.send_key_event(key, state, False)

    def send_key_event(self, key: int, state: bool, finish: bool) -> None:
        self._send_key_event(key, state)
        if state and finish and (key not in EvdevModifiers.ALL and key != ecodes.KEY_SYSRQ):
            # Считаем что PrintScreen это модификатор для Alt+SysRq+...
            # По-хорошему надо учитывать факт нажатия на Alt, но можно и забить.
            self._send_key_event(key, False)
        self.__bump_activity()

    def _send_key_event(self, key: int, state: bool) -> None:
        raise NotImplementedError

    # =====

    def send_mouse_button_event(self, button: int, state: bool) -> None:
        self._send_mouse_button_event(button, state)
        self.__bump_activity()

    def _send_mouse_button_event(self, button: int, state: bool) -> None:
        raise NotImplementedError

    # =====

    def send_mouse_move_event(self, to_x: int, to_y: int) -> None:
        self.__j_last_x = to_x
        self.__j_last_y = to_y
        if self.__mouse_x_range != MouseRange.RANGE:
            to_x = MouseRange.remap(to_x, *self.__mouse_x_range)
        if self.__mouse_y_range != MouseRange.RANGE:
            to_y = MouseRange.remap(to_y, *self.__mouse_y_range)
        self._send_mouse_move_event(to_x, to_y)
        self.__bump_activity()

    def _send_mouse_move_event(self, to_x: int, to_y: int) -> None:
        _ = to_x  # XXX: NotImplementedError
        _ = to_y

    # =====

    def send_mouse_relative_events(self, deltas: Iterable[tuple[int, int]], squash: bool) -> None:
        self.__process_mouse_delta_event(deltas, squash, self.send_mouse_relative_event)

    def send_mouse_relative_event(self, delta_x: int, delta_y: int) -> None:
        self._send_mouse_relative_event(delta_x, delta_y)
        self.__bump_activity()

    def _send_mouse_relative_event(self, delta_x: int, delta_y: int) -> None:
        _ = delta_x  # XXX: NotImplementedError
        _ = delta_y

    # =====

    def send_mouse_wheel_events(self, deltas: Iterable[tuple[int, int]], squash: bool) -> None:
        self.__process_mouse_delta_event(deltas, squash, self.send_mouse_wheel_event)

    def send_mouse_wheel_event(self, delta_x: int, delta_y: int) -> None:
        self._send_mouse_wheel_event(delta_x, delta_y)
        self.__bump_activity()

    def _send_mouse_wheel_event(self, delta_x: int, delta_y: int) -> None:
        raise NotImplementedError

    # =====

    def clear_events(self) -> None:
        self._clear_events()  # Don't bump activity here

    def _clear_events(self) -> None:
        raise NotImplementedError

    # =====

    def __process_mouse_delta_event(
        self,
        deltas: Iterable[tuple[int, int]],
        squash: bool,
        handler: Callable[[int, int], None],
    ) -> None:

        if squash:
            prev = (0, 0)
            for cur in deltas:
                if abs(prev[0] + cur[0]) > 127 or abs(prev[1] + cur[1]) > 127:
                    handler(*prev)
                    prev = cur
                else:
                    prev = (prev[0] + cur[0], prev[1] + cur[1])
            if prev[0] or prev[1]:
                handler(*prev)
        else:
            for xy in deltas:
                handler(*xy)

    def __bump_activity(self) -> None:
        self.__j_activity_ts = int(time.monotonic())

    def _set_jiggler_absolute(self, absolute: bool) -> None:
        self.__j_absolute = absolute

    def _set_jiggler_active(self, active: bool) -> None:
        if self.__j_enabled:
            self.__j_active = active

    def _get_jiggler_state(self) -> dict:
        return {
            "jiggler": {
                "enabled":  self.__j_enabled,
                "active":   self.__j_active,
                "interval": self.__j_interval,
            },
        }

    # =====

    async def systask(self) -> None:
        while True:
            if self.__j_active and (self.__j_activity_ts + self.__j_interval < int(time.monotonic())):
                if self.__j_absolute:
                    (x, y) = (self.__j_last_x, self.__j_last_y)
                    for move in [100, -100, 100, -100, 0]:
                        self.send_mouse_move_event(MouseRange.normalize(x + move), MouseRange.normalize(y + move))
                        await asyncio.sleep(0.1)
                else:
                    for move in [10, -10, 10, -10]:
                        self.send_mouse_relative_event(move, move)
                        await asyncio.sleep(0.1)
            await asyncio.sleep(1)


# =====
def get_hid_class(name: str) -> type[BaseHid]:
    return get_plugin_class("hid", name)  # type: ignore
