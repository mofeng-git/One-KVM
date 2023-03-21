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

from ....logging import get_logger

from .... import tools
from .... import aiotools
from .... import aiomulti
from .... import aioproc

from ....yamlconf import Option

from ....validators.basic import valid_bool
from ....validators.basic import valid_int_f0
from ....validators.basic import valid_int_f1
from ....validators.basic import valid_float_f01
from ....validators.os import valid_abs_path
from ....validators.hw import valid_tty_speed

from .. import BaseHid

from .tty import TTY
from .mouse import Mouse
from .keyboard import Keyboard

from .tty import get_info


class _ResError(Exception):
    def __init__(self, msg: str) -> None:
        super().__init__(msg)
        self.msg = msg


# =====
class Plugin(BaseHid, multiprocessing.Process):  # pylint: disable=too-many-instance-attributes
    def __init__(  # pylint: disable=too-many-arguments,super-init-not-called
        self,
        device_path: str,
        speed: int,
        read_timeout: float,
        read_retries: int,
        common_retries: int,
        retries_delay: float,
        errors_threshold: int,
        noop: bool,
    ) -> None:

        multiprocessing.Process.__init__(self, daemon=True)

        self.__device_path = device_path
        self.__speed = speed
        self.__read_timeout = read_timeout
        self.__read_retries = read_retries
        self.__common_retries = common_retries
        self.__retries_delay = retries_delay
        self.__errors_threshold = errors_threshold
        self.__noop = noop

        self.__reset_required_event = multiprocessing.Event()
        self.__cmd_queue: "multiprocessing.Queue[list]" = multiprocessing.Queue()

        self.__notifier = aiomulti.AioProcessNotifier()
        self.__state_flags = aiomulti.AioSharedFlags({
            "online": 0,
            "busy": 0,
            "status": 0,
        }, self.__notifier, type=int)

        self.__stop_event = multiprocessing.Event()
        self.__tty = TTY(device_path, speed, read_timeout)
        self.__keyboard = Keyboard()
        self.__mouse = Mouse()

    @classmethod
    def get_plugin_options(cls) -> dict:
        return {
            "device":           Option("/dev/kvmd-hid", type=valid_abs_path, unpack_as="device_path"),
            "speed":            Option(9600,  type=valid_tty_speed),
            "read_timeout":     Option(0.3,   type=valid_float_f01),
            "read_retries":     Option(5,     type=valid_int_f1),
            "common_retries":   Option(5,     type=valid_int_f1),
            "retries_delay":    Option(0.5,   type=valid_float_f01),
            "errors_threshold": Option(5,     type=valid_int_f0),
            "noop":             Option(False, type=valid_bool),
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

    def send_mouse_button_event(self, button: str, state: bool) -> None:
        self.__queue_cmd(self.__mouse.process_button(button, state))

    def send_mouse_move_event(self, to_x: int, to_y: int) -> None:
        self.__queue_cmd(self.__mouse.process_move(to_x, to_y))

    def send_mouse_wheel_event(self, delta_x: int, delta_y: int) -> None:
        self.__queue_cmd(self.__mouse.process_wheel(delta_x, delta_y))

    def send_mouse_relative_event(self, delta_x: int, delta_y: int) -> None:
        self.__queue_cmd(self.__mouse.process_relative(delta_x, delta_y))

    def set_params(self, keyboard_output: (str | None)=None, mouse_output: (str | None)=None) -> None:
        if mouse_output is not None:
            get_logger(0).info("HID : mouse output = %s", mouse_output)
            self.__mouse.set_absolute(mouse_output == "usb")
            self.__notifier.notify()

    def set_connected(self, connected: bool) -> None:
        pass

    def clear_events(self) -> None:
        tools.clear_queue(self.__cmd_queue)

    def __queue_cmd(self, cmd: list[int], clear: bool=False) -> None:
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
                # self.__tty.connect()
                self.__hid_loop()
            except Exception:
                logger.exception("Unexpected error in the run loop")
                time.sleep(1)

    def __hid_loop(self) -> None:
        while not self.__stop_event.is_set():
            try:

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
                        self.__process_cmd(get_info())
                    else:
                        self.__process_cmd(cmd)
            except Exception:
                self.clear_events()
                get_logger(0).exception("Unexpected error in the HID loop")
                time.sleep(2)

    def __process_cmd(self, cmd: list[int]) -> bool:  # pylint: disable=too-many-branches
        error_retval = False
        try:
            res = self.__tty.send(cmd)
            # get_logger(0).info(f"HID response = {res}")
            if len(res) < 4:
                raise _ResError("No response from HID - might be disconnected")

            if not self.__tty.check_res(res):
                raise _ResError("Invalid response checksum ...")

            # Response Error
            if res[4] == 1 and res[5] != 0:
                raise _ResError("Command error code = " + str(res[5]))

            # get_info response
            if res[3] == 0x81:
                self.__keyboard.set_leds(res[7])
                self.__notifier.notify()

            self.__set_state_online(True)
            return True

        except _ResError as err:
            self.__set_state_online(False)
            get_logger(0).info(err)
            error_retval = False
            time.sleep(2)

        return error_retval

    def __set_state_online(self, online: bool) -> None:
        self.__state_flags.update(online=int(online))

    def __set_state_busy(self, busy: bool) -> None:
        self.__state_flags.update(busy=int(busy))
