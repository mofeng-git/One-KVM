# ========================================================================== #
#                                                                            #
#    KVMD - The main Pi-KVM daemon.                                          #
#                                                                            #
#    Copyright (C) 2018-2021  Maxim Devaev <mdevaev@gmail.com>               #
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


from typing import Tuple
from typing import Dict
from typing import Iterable
from typing import AsyncGenerator
from typing import Optional
from typing import Any

from .... import tools
from .... import aiomulti

from ....yamlconf import Option

from ....validators.basic import valid_bool
from ....validators.basic import valid_int_f1
from ....validators.basic import valid_float_f01
from ....validators.os import valid_abs_path

from .. import BaseHid

from .usb import UsbDeviceController
from .keyboard import KeyboardProcess
from .mouse import MouseProcess


# =====
class Plugin(BaseHid):  # pylint: disable=too-many-instance-attributes
    def __init__(  # pylint: disable=super-init-not-called
        self,
        keyboard: Dict[str, Any],
        mouse: Dict[str, Any],
        mouse_alt: Dict[str, Any],
        noop: bool,
        udc: str,  # XXX: Not from options, see /kvmd/apps/kvmd/__init__.py for details
    ) -> None:

        self.__notifier = aiomulti.AioProcessNotifier()

        self.__udc = UsbDeviceController(udc)

        common = {
            "udc": self.__udc,
            "noop": noop,
            "notifier": self.__notifier,
        }

        self.__keyboard_proc = KeyboardProcess(**common, **keyboard)
        self.__mouse_current = self.__mouse_proc = MouseProcess(**common, **mouse)

        self.__mouse_alt_proc: Optional[MouseProcess] = None
        self.__output_to_mouse: Dict[str, MouseProcess] = {}
        self.__mouse_to_output: Dict[MouseProcess, str] = {}
        if mouse_alt["device_path"]:
            self.__mouse_alt_proc = MouseProcess(absolute=(not mouse["absolute"]), **common, **mouse_alt)
            self.__output_to_mouse = {
                "usb": (self.__mouse_proc if mouse["absolute"] else self.__mouse_alt_proc),
                "usb_rel": (self.__mouse_alt_proc if mouse["absolute"] else self.__mouse_proc),
            }
            self.__mouse_to_output = tools.swapped_kvs(self.__output_to_mouse)

    @classmethod
    def get_plugin_options(cls) -> Dict:
        return {
            "keyboard": {
                "device":         Option("",  type=valid_abs_path, unpack_as="device_path"),
                "select_timeout": Option(0.1, type=valid_float_f01),
                "queue_timeout":  Option(0.1, type=valid_float_f01),
                "write_retries":  Option(150, type=valid_int_f1),
            },
            "mouse": {
                "device":           Option("",   type=valid_abs_path, unpack_as="device_path"),
                "select_timeout":   Option(0.1,  type=valid_float_f01),
                "queue_timeout":    Option(0.1,  type=valid_float_f01),
                "write_retries":    Option(150,  type=valid_int_f1),
                "absolute":         Option(True, type=valid_bool),
                "horizontal_wheel": Option(True, type=valid_bool),
            },
            "mouse_alt": {
                "device":           Option("",   type=valid_abs_path, if_empty="", unpack_as="device_path"),
                "select_timeout":   Option(0.1,  type=valid_float_f01),
                "queue_timeout":    Option(0.1,  type=valid_float_f01),
                "write_retries":    Option(150,  type=valid_int_f1),
                # No absolute option here, initialized by (not mouse.absolute)
                "horizontal_wheel": Option(True, type=valid_bool),
            },
            "noop": Option(False, type=valid_bool),
        }

    def sysprep(self) -> None:
        self.__udc.find()
        self.__keyboard_proc.start()
        self.__mouse_proc.start()
        if self.__mouse_alt_proc:
            self.__mouse_alt_proc.start()

    async def get_state(self) -> Dict:
        keyboard_state = await self.__keyboard_proc.get_state()
        mouse_state = await self.__mouse_current.get_state()
        return {
            "online": True,
            "busy": False,
            "connected": None,
            "keyboard": {
                "online": keyboard_state["online"],
                "leds": {
                    "caps": keyboard_state["caps"],
                    "scroll": keyboard_state["scroll"],
                    "num": keyboard_state["num"],
                },
                "outputs": {"available": [], "active": ""},
            },
            "mouse": {
                "outputs": {
                    "available": list(self.__output_to_mouse),
                    "active": (self.__mouse_to_output[self.__mouse_current] if self.__mouse_alt_proc else ""),
                },
                **mouse_state,
            },
        }

    async def poll_state(self) -> AsyncGenerator[Dict, None]:
        prev_state: Dict = {}
        while True:
            state = await self.get_state()
            if state != prev_state:
                yield state
                prev_state = state
            await self.__notifier.wait()

    async def reset(self) -> None:
        self.__keyboard_proc.send_reset_event()
        self.__mouse_current.send_reset_event()

    async def cleanup(self) -> None:
        try:
            self.__keyboard_proc.cleanup()
        finally:
            try:
                self.__mouse_proc.cleanup()
            finally:
                if self.__mouse_alt_proc:
                    self.__mouse_alt_proc.cleanup()

    # =====

    def send_key_events(self, keys: Iterable[Tuple[str, bool]]) -> None:
        self.__keyboard_proc.send_key_events(keys)

    def send_mouse_button_event(self, button: str, state: bool) -> None:
        self.__mouse_current.send_button_event(button, state)

    def send_mouse_move_event(self, to_x: int, to_y: int) -> None:
        self.__mouse_current.send_move_event(to_x, to_y)

    def send_mouse_relative_event(self, delta_x: int, delta_y: int) -> None:
        self.__mouse_current.send_relative_event(delta_x, delta_y)

    def send_mouse_wheel_event(self, delta_x: int, delta_y: int) -> None:
        self.__mouse_current.send_wheel_event(delta_x, delta_y)

    def set_params(self, keyboard_output: Optional[str]=None, mouse_output: Optional[str]=None) -> None:
        _ = keyboard_output
        if mouse_output != self.__mouse_to_output[self.__mouse_current]:
            if mouse_output in self.__output_to_mouse:
                self.__mouse_current.send_clear_event()
                self.__mouse_current = self.__output_to_mouse[mouse_output]
                self.__notifier.notify()

    def clear_events(self) -> None:
        self.__keyboard_proc.send_clear_event()
        self.__mouse_current.send_clear_event()
