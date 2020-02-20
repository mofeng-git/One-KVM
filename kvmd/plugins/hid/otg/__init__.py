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
import concurrent.futures
import multiprocessing
import multiprocessing.queues
import queue
import functools

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
    ) -> None:

        self.__changes_queue: multiprocessing.queues.Queue = multiprocessing.Queue()

        self.__keyboard_proc = KeyboardProcess(noop=noop, changes_queue=self.__changes_queue, **keyboard)
        self.__mouse_proc = MouseProcess(noop=noop, changes_queue=self.__changes_queue, **mouse)

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
            "noop": Option(False, type=valid_bool),
        }

    def start(self) -> None:
        self.__keyboard_proc.start()
        self.__mouse_proc.start()

    def get_state(self) -> Dict:
        keyboard_state = self.__keyboard_proc.get_state()
        mouse_state = self.__mouse_proc.get_state()
        return {
            "online": (keyboard_state["online"] and mouse_state["online"]),
            "keyboard": {"features": {"leds": True}, **keyboard_state},
            "mouse": mouse_state,
        }

    async def poll_state(self) -> AsyncGenerator[Dict, None]:
        loop = asyncio.get_running_loop()
        wait_for_changes = functools.partial(self.__changes_queue.get, timeout=1)
        with concurrent.futures.ThreadPoolExecutor(max_workers=1) as executor:
            prev_state: Dict = {}
            while True:
                state = self.get_state()
                if state != prev_state:
                    yield state
                    prev_state = state
                while True:
                    try:
                        await loop.run_in_executor(executor, wait_for_changes)
                        break
                    except queue.Empty:
                        pass

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

    async def send_mouse_wheel_event(self, delta_x: int, delta_y: int) -> None:
        self.__mouse_proc.send_wheel_event(delta_x, delta_y)

    async def clear_events(self) -> None:
        self.__keyboard_proc.send_clear_event()
        self.__mouse_proc.send_clear_event()
