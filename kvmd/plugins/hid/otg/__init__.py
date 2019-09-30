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

from typing import Dict
from typing import AsyncGenerator
from typing import Any

from ....yamlconf import Option

from ....validators.basic import valid_bool
from ....validators.basic import valid_int_f1
from ....validators.basic import valid_float_f01

from ....validators.os import valid_abs_path

from .. import BaseHid

from .keyboard import KeyboardProcess
from .mouse import MouseProcess


# =====
class Plugin(BaseHid):
    def __init__(  # pylint: disable=super-init-not-called
        self,
        keyboard: Dict[str, Any],
        mouse: Dict[str, Any],
        noop: bool,
        state_poll: float,
    ) -> None:

        self.__keyboard_proc = KeyboardProcess(noop=noop, **keyboard)
        self.__mouse_proc = MouseProcess(noop=noop, **mouse)

        self.__state_poll = state_poll

        self.__lock = asyncio.Lock()

    @classmethod
    def get_plugin_options(cls) -> Dict:
        return {
            "keyboard": {
                "device":              Option("",  type=valid_abs_path, unpack_as="device_path"),
                "select_timeout":      Option(1.0, type=valid_float_f01),
                "write_retries":       Option(5,   type=valid_int_f1),
                "write_retries_delay": Option(0.1, type=valid_float_f01),
            },

            "mouse": {
                "device":              Option("",  type=valid_abs_path, unpack_as="device_path"),
                "select_timeout":      Option(1.0, type=valid_float_f01),
                "write_retries":       Option(5,   type=valid_int_f1),
                "write_retries_delay": Option(0.1, type=valid_float_f01),
            },

            "noop":       Option(False,  type=valid_bool),
            "state_poll": Option(0.1, type=valid_float_f01),
        }

    def start(self) -> None:
        self.__keyboard_proc.start()
        self.__mouse_proc.start()

    def get_state(self) -> Dict:
        keyboard_online = self.__keyboard_proc.is_online()
        mouse_online = self.__mouse_proc.is_online()
        return {
            "online": (keyboard_online and mouse_online),
            "keyboard": {"online": keyboard_online},
            "mouse": {"online": mouse_online},
        }

    async def poll_state(self) -> AsyncGenerator[Dict, None]:
        prev_state: Dict = {}
        while self.__keyboard_proc.is_alive() and self.__mouse_proc.is_alive():
            state = self.get_state()
            if state != prev_state:
                yield self.get_state()
                prev_state = state
            await asyncio.sleep(self.__state_poll)

    async def reset(self) -> None:
        self.__keyboard_proc.send_reset_event()
        self.__mouse_proc.send_reset_event()

    async def cleanup(self) -> None:
        try:
            self.__keyboard_proc.cleanup()
        finally:
            self.__mouse_proc.cleanup()

    # =====

    async def send_key_event(self, key: str, state: bool) -> None:
        self.__keyboard_proc.send_key_event(key, state)

    async def send_mouse_button_event(self, button: str, state: bool) -> None:
        self.__mouse_proc.send_button_event(button, state)

    async def send_mouse_move_event(self, to_x: int, to_y: int) -> None:
        self.__mouse_proc.send_move_event(to_x, to_y)

    async def send_mouse_wheel_event(self, delta_y: int) -> None:
        self.__mouse_proc.send_wheel_event(delta_y)

    async def clear_events(self) -> None:
        self.__keyboard_proc.send_clear_event()
        self.__mouse_proc.send_clear_event()
