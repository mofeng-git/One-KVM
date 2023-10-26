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


import multiprocessing
import time

from typing import Iterable
from typing import AsyncGenerator
from typing import Any

from ....logging import get_logger

from ....yamlconf import Option

from ....validators.basic import valid_bool
from ....validators.basic import valid_stripped_string_not_empty
from ....validators.basic import valid_int_f1
from ....validators.basic import valid_float_f01

from .... import aiotools
from .... import aiomulti
from .... import aioproc

from .. import BaseHid

from ..otg.events import ResetEvent
from ..otg.events import make_keyboard_event
from ..otg.events import MouseButtonEvent
from ..otg.events import MouseRelativeEvent
from ..otg.events import MouseWheelEvent

from .sdp import make_sdp_record
from .bluez import BluezIface
from .server import BtServer


# =====
class Plugin(BaseHid):  # pylint: disable=too-many-instance-attributes
    # https://github.com/SySS-Research/bluetooth-keyboard-emulator
    # https://github.com/nutki/bt-keyboard-switcher
    # https://gist.github.com/whitelynx/9f9bd4cb266b3924c64dfdff14bce2e8
    # https://archlinuxarm.org/forum/viewtopic.php?f=67&t=14244

    def __init__(  # pylint: disable=too-many-arguments,too-many-locals
        self,

        manufacturer: str,
        product: str,
        description: str,

        iface: str,
        alias: str,

        pairing_required: bool,
        auth_required: bool,
        control_public: bool,
        unpair_on_close: bool,

        max_clients: int,
        socket_timeout: float,
        select_timeout: float,

        jiggler: dict[str, Any],
    ) -> None:

        super().__init__(**jiggler)
        self._set_jiggler_absolute(False)

        self.__proc: (multiprocessing.Process | None) = None
        self.__stop_event = multiprocessing.Event()

        self.__notifier = aiomulti.AioProcessNotifier()

        self.__server = BtServer(
            iface=BluezIface(
                iface=iface,
                alias=alias,
                sdp_record=make_sdp_record(manufacturer, product, description),
                pairing_required=pairing_required,
                auth_required=auth_required,
            ),
            control_public=control_public,
            unpair_on_close=unpair_on_close,
            max_clients=max_clients,
            socket_timeout=socket_timeout,
            select_timeout=select_timeout,
            notifier=self.__notifier,
            stop_event=self.__stop_event,
        )

    @classmethod
    def get_plugin_options(cls) -> dict:
        return {
            "manufacturer": Option("PiKVM"),
            "product":      Option("HID Device"),
            "description":  Option("Bluetooth Keyboard & Mouse"),

            "iface": Option("hci0", type=valid_stripped_string_not_empty),
            "alias": Option("PiKVM HID"),

            "pairing_required": Option(True,  type=valid_bool),
            "auth_required":    Option(False, type=valid_bool),
            "control_public":   Option(True,  type=valid_bool),
            "unpair_on_close":  Option(True,  type=valid_bool),

            "max_clients":    Option(1,   type=valid_int_f1),
            "socket_timeout": Option(5.0, type=valid_float_f01),
            "select_timeout": Option(1.0, type=valid_float_f01),

            **cls._get_jiggler_options(),
        }

    def sysprep(self) -> None:
        get_logger(0).info("Starting HID daemon ...")
        self.__proc = multiprocessing.Process(target=self.__server_worker, daemon=True)
        self.__proc.start()

    async def get_state(self) -> dict:
        state = await self.__server.get_state()
        outputs: dict = {"available": [], "active": ""}
        return {
            "online": True,
            "busy": False,
            "connected": None,
            "keyboard": {
                "online": state["online"],
                "leds": {
                    "caps": state["caps"],
                    "scroll": state["scroll"],
                    "num": state["num"],
                },
                "outputs": outputs,
            },
            "mouse": {
                "online": state["online"],
                "absolute": False,
                "outputs": outputs,
            },
            **self._get_jiggler_state(),
        }

    async def poll_state(self) -> AsyncGenerator[dict, None]:
        prev_state: dict = {}
        while True:
            state = await self.get_state()
            if state != prev_state:
                yield state
                prev_state = state
            await self.__notifier.wait()

    async def reset(self) -> None:
        self.clear_events()
        self.__server.queue_event(ResetEvent())

    @aiotools.atomic_fg
    async def cleanup(self) -> None:
        if self.__proc is not None:
            if self.__proc.is_alive():
                get_logger(0).info("Stopping HID daemon ...")
                self.__stop_event.set()
            if self.__proc.is_alive() or self.__proc.exitcode is not None:
                self.__proc.join()

    # =====

    def send_key_events(self, keys: Iterable[tuple[str, bool]]) -> None:
        for (key, state) in keys:
            self.__server.queue_event(make_keyboard_event(key, state))
            self._bump_activity()

    def send_mouse_button_event(self, button: str, state: bool) -> None:
        self.__server.queue_event(MouseButtonEvent(button, state))
        self._bump_activity()

    def send_mouse_relative_event(self, delta_x: int, delta_y: int) -> None:
        self.__server.queue_event(MouseRelativeEvent(delta_x, delta_y))
        self._bump_activity()

    def send_mouse_wheel_event(self, delta_x: int, delta_y: int) -> None:
        self.__server.queue_event(MouseWheelEvent(delta_x, delta_y))
        self._bump_activity()

    def clear_events(self) -> None:
        self.__server.clear_events()
        self._bump_activity()

    def set_params(
        self,
        keyboard_output: (str | None)=None,
        mouse_output: (str | None)=None,
        jiggler: (bool | None)=None,
    ) -> None:

        _ = keyboard_output
        _ = mouse_output
        if jiggler is not None:
            self._set_jiggler_active(jiggler)
            self.__notifier.notify()

    # =====

    def __server_worker(self) -> None:  # pylint: disable=too-many-branches
        logger = aioproc.settle("HID", "hid")
        while not self.__stop_event.is_set():
            try:
                self.__server.run()
            except Exception:
                logger.exception("Unexpected HID error")
                time.sleep(5)
