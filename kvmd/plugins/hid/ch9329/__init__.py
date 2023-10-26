# ========================================================================== #
#                                                                            #
#    KVMD - The main PiKVM daemon.                                           #
#                                                                            #
#    Copyright (C) 2018-2022  Maxim Devaev <mdevaev@gmail.com>               #
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
import queue
import time

from typing import Iterable
from typing import AsyncGenerator
from typing import Any

from ....logging import get_logger

from .... import tools
from .... import aiotools
from .... import aiomulti
from .... import aioproc

from ....yamlconf import Option

from ....validators.basic import valid_float_f01
from ....validators.os import valid_abs_path
from ....validators.hw import valid_tty_speed

from .. import BaseHid

from .chip import ChipResponseError
from .chip import ChipConnection
from .chip import Chip
from .mouse import Mouse
from .keyboard import Keyboard


# =====
class Plugin(BaseHid, multiprocessing.Process):  # pylint: disable=too-many-instance-attributes
    def __init__(  # pylint: disable=too-many-arguments,super-init-not-called
        self,
        device_path: str,
        speed: int,
        read_timeout: float,
        jiggler: dict[str, Any],
    ) -> None:

        BaseHid.__init__(self, **jiggler)
        multiprocessing.Process.__init__(self, daemon=True)

        self.__device_path = device_path
        self.__speed = speed
        self.__read_timeout = read_timeout

        self.__reset_required_event = multiprocessing.Event()
        self.__cmd_queue: "multiprocessing.Queue[bytes]" = multiprocessing.Queue()

        self.__notifier = aiomulti.AioProcessNotifier()
        self.__state_flags = aiomulti.AioSharedFlags({
            "online": 0,
            "busy": 0,
            "status": 0,
        }, self.__notifier, type=int)

        self.__stop_event = multiprocessing.Event()
        self.__chip = Chip(device_path, speed, read_timeout)
        self.__keyboard = Keyboard()
        self.__mouse = Mouse()

    @classmethod
    def get_plugin_options(cls) -> dict:
        return {
            "device":       Option("/dev/kvmd-hid", type=valid_abs_path, unpack_as="device_path"),
            "speed":        Option(9600, type=valid_tty_speed),
            "read_timeout": Option(0.3,  type=valid_float_f01),
            **cls._get_jiggler_options(),
        }

    def sysprep(self) -> None:
        get_logger(0).info("Starting HID daemon ...")
        self.start()

    async def get_state(self) -> dict:
        state = await self.__state_flags.get()
        absolute = self.__mouse.is_absolute()
        leds = await self.__keyboard.get_leds()
        return {
            "online": state["online"],
            "busy": False,
            "connected": None,
            "keyboard": {
                "online": state["online"],
                "leds": leds,
                "outputs": {"available": [], "active": ""},
            },
            "mouse": {
                "online": state["online"],
                "absolute": absolute,
                "outputs": {
                    "available": ["usb", "usb_rel"],
                    "active": ("usb" if absolute else "usb_rel"),
                },
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
        self.__reset_required_event.set()

    @aiotools.atomic_fg
    async def cleanup(self) -> None:
        if self.is_alive():
            get_logger(0).info("Stopping HID daemon ...")
            self.__stop_event.set()
        if self.is_alive() or self.exitcode is not None:
            self.join()

    # =====

    def send_key_events(self, keys: Iterable[tuple[str, bool]]) -> None:
        for (key, state) in keys:
            self.__queue_cmd(self.__keyboard.process_key(key, state))
            self._bump_activity()

    def send_mouse_button_event(self, button: str, state: bool) -> None:
        self.__queue_cmd(self.__mouse.process_button(button, state))
        self._bump_activity()

    def send_mouse_move_event(self, to_x: int, to_y: int) -> None:
        self.__queue_cmd(self.__mouse.process_move(to_x, to_y))
        self._bump_activity()

    def send_mouse_wheel_event(self, delta_x: int, delta_y: int) -> None:
        self.__queue_cmd(self.__mouse.process_wheel(delta_x, delta_y))
        self._bump_activity()

    def send_mouse_relative_event(self, delta_x: int, delta_y: int) -> None:
        self.__queue_cmd(self.__mouse.process_relative(delta_x, delta_y))
        self._bump_activity()

    def set_params(
        self,
        keyboard_output: (str | None)=None,
        mouse_output: (str | None)=None,
        jiggler: (bool | None)=None,
    ) -> None:

        _ = keyboard_output
        if mouse_output is not None:
            get_logger(0).info("HID : mouse output = %s", mouse_output)
            absolute = (mouse_output == "usb")
            self.__mouse.set_absolute(absolute)
            self._set_jiggler_absolute(absolute)
            self.__notifier.notify()
        if jiggler is not None:
            self._set_jiggler_active(jiggler)
            self.__notifier.notify()

    def set_connected(self, connected: bool) -> None:
        pass

    def clear_events(self) -> None:
        tools.clear_queue(self.__cmd_queue)

    def __queue_cmd(self, cmd: bytes, clear: bool=False) -> None:
        if not self.__stop_event.is_set():
            if clear:
                # FIXME: Если очистка производится со стороны процесса хида, то возможна гонка между
                # очисткой и добавлением нового события. Неприятно, но не смертельно.
                # Починить блокировкой после перехода на асинхронные очереди.
                tools.clear_queue(self.__cmd_queue)
            self.__cmd_queue.put_nowait(cmd)

    def run(self) -> None:  # pylint: disable=too-many-branches
        logger = aioproc.settle("HID", "hid")
        while not self.__stop_event.is_set():
            try:
                self.__hid_loop()
            except Exception:
                logger.exception("Unexpected error in the run loop")
                time.sleep(1)

    def __hid_loop(self) -> None:
        while not self.__stop_event.is_set():
            try:
                with self.__chip.connected() as conn:
                    while not (self.__stop_event.is_set() and self.__cmd_queue.qsize() == 0):
                        if self.__reset_required_event.is_set():
                            try:
                                self.__set_state_busy(True)
                                # self.__process_request(conn, RESET)
                            finally:
                                self.__reset_required_event.clear()
                        try:
                            cmd = self.__cmd_queue.get(timeout=0.1)
                            # get_logger(0).info(f"HID : cmd = {cmd}")
                        except queue.Empty:
                            self.__process_cmd(conn, b"")
                        else:
                            self.__process_cmd(conn, cmd)
            except Exception:
                self.clear_events()
                get_logger(0).exception("Unexpected error in the HID loop")
                time.sleep(2)

    def __process_cmd(self, conn: ChipConnection, cmd: bytes) -> bool:  # pylint: disable=too-many-branches
        try:
            led_byte = conn.xfer(cmd)
        except ChipResponseError as err:
            self.__set_state_online(False)
            get_logger(0).info(err)
            time.sleep(2)
        else:
            if led_byte >= 0:
                self.__keyboard.set_leds(led_byte)
                self.__notifier.notify()
            self.__set_state_online(True)
            return True
        return False

    def __set_state_online(self, online: bool) -> None:
        self.__state_flags.update(online=int(online))

    def __set_state_busy(self, busy: bool) -> None:
        self.__state_flags.update(busy=int(busy))
