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


import os
import multiprocessing
import contextlib
import queue
import time

from typing import Tuple
from typing import List
from typing import Dict
from typing import Iterable
from typing import Generator
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
from ....validators.hw import valid_gpio_pin_optional

from .. import BaseHid

from .gpio import Gpio

from .proto import REQUEST_PING
from .proto import REQUEST_REPEAT
from .proto import RESPONSE_LEGACY_OK
from .proto import KEYBOARD_CODES_TO_NAMES
from .proto import MOUSE_CODES_TO_NAMES
from .proto import BaseEvent
from .proto import SetKeyboardOutputEvent
from .proto import SetMouseOutputEvent
from .proto import ClearEvent
from .proto import KeyEvent
from .proto import MouseButtonEvent
from .proto import MouseMoveEvent
from .proto import MouseRelativeEvent
from .proto import MouseWheelEvent
from .proto import check_response


# =====
class _RequestError(Exception):
    def __init__(self, msg: str) -> None:
        super().__init__(msg)
        self.msg = msg


class _PermRequestError(_RequestError):
    pass


class _TempRequestError(_RequestError):
    pass


# =====
class BasePhyConnection:
    def send(self, request: bytes) -> bytes:
        raise NotImplementedError


class BasePhy:
    def has_device(self) -> bool:
        raise NotImplementedError

    @contextlib.contextmanager
    def connected(self) -> Generator[BasePhyConnection, None, None]:
        raise NotImplementedError


class BaseMcuHid(BaseHid, multiprocessing.Process):  # pylint: disable=too-many-instance-attributes
    def __init__(  # pylint: disable=too-many-arguments,super-init-not-called
        self,
        phy: BasePhy,

        reset_pin: int,
        reset_inverted: bool,
        reset_delay: float,

        read_retries: int,
        common_retries: int,
        retries_delay: float,
        errors_threshold: int,
        noop: bool,
    ) -> None:

        multiprocessing.Process.__init__(self, daemon=True)

        self.__read_retries = read_retries
        self.__common_retries = common_retries
        self.__retries_delay = retries_delay
        self.__errors_threshold = errors_threshold
        self.__noop = noop

        self.__phy = phy
        self.__gpio = Gpio(reset_pin, reset_inverted, reset_delay)

        self.__events_queue: "multiprocessing.Queue[BaseEvent]" = multiprocessing.Queue()

        self.__notifier = aiomulti.AioProcessNotifier()
        self.__state_flags = aiomulti.AioSharedFlags({
            "online": 0,
            "status": 0,
        }, self.__notifier, type=int)

        self.__stop_event = multiprocessing.Event()

    @classmethod
    def get_plugin_options(cls) -> Dict:
        return {
            "reset_pin":      Option(-1,    type=valid_gpio_pin_optional),
            "reset_inverted": Option(False, type=valid_bool),
            "reset_delay":    Option(0.1,   type=valid_float_f01),

            "read_retries":     Option(10,     type=valid_int_f1),
            "common_retries":   Option(100,    type=valid_int_f1),
            "retries_delay":    Option(0.1,    type=valid_float_f01),
            "errors_threshold": Option(5,      type=valid_int_f0),
            "noop":             Option(False,  type=valid_bool),
        }

    def sysprep(self) -> None:
        self.__gpio.open()
        get_logger(0).info("Starting HID daemon ...")
        self.start()

    async def get_state(self) -> Dict:
        state = await self.__state_flags.get()
        online = bool(state["online"])
        pong = (state["status"] >> 16) & 0xFF
        outputs = (state["status"] >> 8) & 0xFF
        features = state["status"] & 0xFF

        absolute = True
        if online and (outputs & 0b00111000) in [0b00010000, 0b00011000]:
            absolute = False

        keyboard_outputs: Dict = {"available": {}, "active": ""}
        mouse_outputs: Dict = {"available": {}, "active": ""}

        if outputs & 0b10000000:  # Dynamic
            if features & 0b00000001:  # USB
                keyboard_outputs["available"]["usb"] = {"name": "USB"}
                mouse_outputs["available"]["usb"] = {"name": "USB", "absolute": True}
                mouse_outputs["available"]["usb_rel"] = {"name": "USB Relative", "absolute": False}

            if features & 0b00000010:  # PS/2
                keyboard_outputs["available"]["ps2"] = {"name": "PS/2"}
                mouse_outputs["available"]["ps2"] = {"name": "PS/2"}

            active = KEYBOARD_CODES_TO_NAMES.get(outputs & 0b00000111, "")
            if active in keyboard_outputs["available"]:
                keyboard_outputs["active"] = active

            active = MOUSE_CODES_TO_NAMES.get(outputs & 0b00111000, "")
            if active in mouse_outputs["available"]:
                mouse_outputs["active"] = active

        return {
            "online": online,
            "keyboard": {
                "online": (online and not (pong & 0b00001000)),
                "leds": {
                    "caps":   bool(pong & 0b00000001),
                    "scroll": bool(pong & 0b00000010),
                    "num":    bool(pong & 0b00000100),
                },
                "outputs": keyboard_outputs,
            },
            "mouse": {
                "online": (online and not (pong & 0b00010000)),
                "absolute": absolute,
                "outputs": mouse_outputs,
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

    @aiotools.atomic
    async def reset(self) -> None:
        await self.__gpio.reset()

    @aiotools.atomic
    async def cleanup(self) -> None:
        logger = get_logger(0)
        try:
            if self.is_alive():
                logger.info("Stopping HID daemon ...")
                self.__stop_event.set()
            if self.exitcode is not None:
                self.join()
            if self.__phy.has_device():
                get_logger().info("Clearing HID events ...")
                try:
                    with self.__phy.connected() as conn:
                        self.__process_request(conn, ClearEvent().make_request())
                except Exception:
                    logger.exception("Can't clear HID events")
        finally:
            self.__gpio.close()

    # =====

    def send_key_events(self, keys: Iterable[Tuple[str, bool]]) -> None:
        for (key, state) in keys:
            self.__queue_event(KeyEvent(key, state))

    def send_mouse_button_event(self, button: str, state: bool) -> None:
        self.__queue_event(MouseButtonEvent(button, state))

    def send_mouse_move_event(self, to_x: int, to_y: int) -> None:
        self.__queue_event(MouseMoveEvent(to_x, to_y))

    def send_mouse_relative_event(self, delta_x: int, delta_y: int) -> None:
        self.__queue_event(MouseRelativeEvent(delta_x, delta_y))

    def send_mouse_wheel_event(self, delta_x: int, delta_y: int) -> None:
        self.__queue_event(MouseWheelEvent(delta_x, delta_y))

    def set_keyboard_output(self, output: str) -> None:
        # FIXME: Если очистка производится со стороны процесса хида, то возможна гонка между
        # очисткой и добавлением нового события. Неприятно, но не смертельно.
        # Починить блокировкой после перехода на асинхронные очереди.
        tools.clear_queue(self.__events_queue)
        self.__queue_event(SetKeyboardOutputEvent(output))

    def set_mouse_output(self, output: str) -> None:
        tools.clear_queue(self.__events_queue)
        self.__queue_event(SetMouseOutputEvent(output))

    def clear_events(self) -> None:
        tools.clear_queue(self.__events_queue)
        self.__queue_event(ClearEvent())

    def __queue_event(self, event: BaseEvent) -> None:
        if not self.__stop_event.is_set():
            self.__events_queue.put_nowait(event)

    def run(self) -> None:  # pylint: disable=too-many-branches
        logger = get_logger(0)

        logger.info("Started HID pid=%d", os.getpid())
        aioproc.ignore_sigint()
        aioproc.rename_process("hid")

        while not self.__stop_event.is_set():
            try:
                if self.__phy.has_device():
                    with self.__phy.connected() as conn:
                        while not (self.__stop_event.is_set() and self.__events_queue.qsize() == 0):
                            try:
                                event = self.__events_queue.get(timeout=0.1)
                            except queue.Empty:
                                self.__process_request(conn, REQUEST_PING)
                            else:
                                if not self.__process_request(conn, event.make_request()):
                                    self.clear_events()
                else:
                    logger.error("Missing HID device")
                    time.sleep(1)
            except Exception:
                self.clear_events()
                logger.exception("Unexpected HID error")
                time.sleep(1)

    def __process_request(self, conn: BasePhyConnection, request: bytes) -> bool:  # pylint: disable=too-many-branches
        logger = get_logger()
        error_messages: List[str] = []
        live_log_errors = False

        common_retries = self.__common_retries
        read_retries = self.__read_retries
        error_retval = False

        while common_retries and read_retries:
            response = (RESPONSE_LEGACY_OK if self.__noop else conn.send(request))
            try:
                if len(response) < 4:
                    read_retries -= 1
                    raise _TempRequestError(f"No response from HID: request={request!r}")

                if not check_response(response):
                    request = REQUEST_REPEAT
                    raise _TempRequestError("Invalid response CRC; requesting response again ...")

                code = response[1]
                if code == 0x48:  # Request timeout  # pylint: disable=no-else-raise
                    raise _TempRequestError(f"Got request timeout from HID: request={request!r}")
                elif code == 0x40:  # CRC Error
                    raise _TempRequestError(f"Got CRC error of request from HID: request={request!r}")
                elif code == 0x45:  # Unknown command
                    raise _PermRequestError(f"HID did not recognize the request={request!r}")
                elif code == 0x24:  # Rebooted?
                    raise _PermRequestError("No previous command state inside HID, seems it was rebooted")
                elif code == 0x20:  # Legacy done
                    self.__set_state_online(True)
                    return True
                elif code & 0x80:  # Pong/Done with state
                    self.__set_state_pong(response)
                    return True
                raise _TempRequestError(f"Invalid response from HID: request={request!r}, response=0x{response!r}")

            except _RequestError as err:
                common_retries -= 1

                if live_log_errors:
                    logger.error(err.msg)
                else:
                    error_messages.append(err.msg)
                    if len(error_messages) > self.__errors_threshold:
                        for msg in error_messages:
                            logger.error(msg)
                        error_messages = []
                        live_log_errors = True

                if isinstance(err, _PermRequestError):
                    error_retval = True
                    break

                self.__set_state_online(False)

                if common_retries and read_retries:
                    time.sleep(self.__retries_delay)

        for msg in error_messages:
            logger.error(msg)
        if not (common_retries and read_retries):
            logger.error("Can't process HID request due many errors: %r", request)
        return error_retval

    def __set_state_online(self, online: bool) -> None:
        self.__state_flags.update(online=int(online))

    def __set_state_pong(self, response: bytes) -> None:
        status = response[1] << 16
        if len(response) > 4:
            status |= (response[2] << 8) | response[3]
        self.__state_flags.update(online=1, status=status)
