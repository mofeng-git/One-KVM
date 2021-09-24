# ========================================================================== #
#                                                                            #
#    KVMD - The main PiKVM daemon.                                           #
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
from typing import Optional

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
from ....validators.hw import valid_gpio_pin_optional

from .. import BaseHid

from .gpio import Gpio

from .proto import REQUEST_PING
from .proto import REQUEST_REPEAT
from .proto import RESPONSE_LEGACY_OK

from .proto import BaseEvent
from .proto import SetKeyboardOutputEvent
from .proto import SetMouseOutputEvent
from .proto import SetConnectedEvent
from .proto import ClearEvent
from .proto import KeyEvent
from .proto import MouseButtonEvent
from .proto import MouseMoveEvent
from .proto import MouseRelativeEvent
from .proto import MouseWheelEvent

from .proto import get_active_keyboard
from .proto import get_active_mouse
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

        gpio_device_path: str,
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
        self.__gpio = Gpio(gpio_device_path, reset_pin, reset_inverted, reset_delay)

        self.__reset_required_event = multiprocessing.Event()
        self.__events_queue: "multiprocessing.Queue[BaseEvent]" = multiprocessing.Queue()

        self.__notifier = aiomulti.AioProcessNotifier()
        self.__state_flags = aiomulti.AioSharedFlags({
            "online": 0,
            "busy": 0,
            "status": 0,
        }, self.__notifier, type=int)

        self.__stop_event = multiprocessing.Event()

    @classmethod
    def get_plugin_options(cls) -> Dict:
        return {
            "gpio_device":    Option("/dev/gpiochip0", type=valid_abs_path, unpack_as="gpio_device_path"),
            "reset_pin":      Option(-1,    type=valid_gpio_pin_optional),
            "reset_inverted": Option(False, type=valid_bool),
            "reset_delay":    Option(0.1,   type=valid_float_f01),

            "read_retries":     Option(5,     type=valid_int_f1),
            "common_retries":   Option(5,     type=valid_int_f1),
            "retries_delay":    Option(0.5,   type=valid_float_f01),
            "errors_threshold": Option(5,     type=valid_int_f0),
            "noop":             Option(False, type=valid_bool),
        }

    def sysprep(self) -> None:
        get_logger(0).info("Starting HID daemon ...")
        self.start()

    async def get_state(self) -> Dict:
        state = await self.__state_flags.get()
        online = bool(state["online"])
        pong = (state["status"] >> 16) & 0xFF
        outputs1 = (state["status"] >> 8) & 0xFF
        outputs2 = state["status"] & 0xFF

        absolute = True
        active_mouse = get_active_mouse(outputs1)
        if online and active_mouse in ["usb_rel", "ps2"]:
            absolute = False

        keyboard_outputs: Dict = {"available": [], "active": ""}
        mouse_outputs: Dict = {"available": [], "active": ""}

        if outputs1 & 0b10000000:  # Dynamic
            if outputs2 & 0b00000001:  # USB
                keyboard_outputs["available"].append("usb")
                mouse_outputs["available"].extend(["usb", "usb_rel"])

            if outputs2 & 0b00000100:  # USB WIN98
                mouse_outputs["available"].append("usb_win98")

            if outputs2 & 0b00000010:  # PS/2
                keyboard_outputs["available"].append("ps2")
                mouse_outputs["available"].append("ps2")

            if keyboard_outputs["available"]:
                keyboard_outputs["available"].append("disabled")

            if mouse_outputs["available"]:
                mouse_outputs["available"].append("disabled")

            active_keyboard = get_active_keyboard(outputs1)
            if active_keyboard in keyboard_outputs["available"]:
                keyboard_outputs["active"] = active_keyboard

            if active_mouse in mouse_outputs["available"]:
                mouse_outputs["active"] = active_mouse

        return {
            "online": online,
            "busy": bool(state["busy"]),
            "connected": (bool(outputs2 & 0b01000000) if outputs2 & 0b10000000 else None),
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

    async def reset(self) -> None:
        self.__reset_required_event.set()

    @aiotools.atomic
    async def cleanup(self) -> None:
        if self.is_alive():
            get_logger(0).info("Stopping HID daemon ...")
            self.__stop_event.set()
        if self.is_alive() or self.exitcode is not None:
            self.join()

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

    def set_params(self, keyboard_output: Optional[str]=None, mouse_output: Optional[str]=None) -> None:
        events: List[BaseEvent] = []
        if keyboard_output is not None:
            events.append(SetKeyboardOutputEvent(keyboard_output))
        if mouse_output is not None:
            events.append(SetMouseOutputEvent(mouse_output))
        for (index, event) in enumerate(events, 1):
            self.__queue_event(event, clear=(index == len(events)))

    def set_connected(self, connected: bool) -> None:
        self.__queue_event(SetConnectedEvent(connected), clear=True)

    def clear_events(self) -> None:
        self.__queue_event(ClearEvent(), clear=True)

    def __queue_event(self, event: BaseEvent, clear: bool=False) -> None:
        if not self.__stop_event.is_set():
            if clear:
                # FIXME: Если очистка производится со стороны процесса хида, то возможна гонка между
                # очисткой и добавлением нового события. Неприятно, но не смертельно.
                # Починить блокировкой после перехода на асинхронные очереди.
                tools.clear_queue(self.__events_queue)
            self.__events_queue.put_nowait(event)

    def run(self) -> None:  # pylint: disable=too-many-branches
        logger = aioproc.settle("HID", "hid")
        while not self.__stop_event.is_set():
            try:
                with self.__gpio:
                    self.__hid_loop()
                    if self.__phy.has_device():
                        logger.info("Clearing HID events ...")
                        try:
                            with self.__phy.connected() as conn:
                                self.__process_request(conn, ClearEvent().make_request())
                        except Exception:
                            logger.exception("Can't clear HID events")
            except Exception:
                logger.exception("Unexpected error in the GPIO loop")
                time.sleep(1)

    def __hid_loop(self) -> None:
        while not self.__stop_event.is_set():
            try:
                if not self.__hid_loop_wait_device():
                    continue
                with self.__phy.connected() as conn:
                    while not (self.__stop_event.is_set() and self.__events_queue.qsize() == 0):
                        if self.__reset_required_event.is_set():
                            try:
                                self.__set_state_busy(True)
                                self.__gpio.reset()
                            finally:
                                self.__reset_required_event.clear()
                        try:
                            event = self.__events_queue.get(timeout=0.1)
                        except queue.Empty:
                            self.__process_request(conn, REQUEST_PING)
                        else:
                            if isinstance(event, (SetKeyboardOutputEvent, SetMouseOutputEvent)):
                                self.__set_state_busy(True)
                            if not self.__process_request(conn, event.make_request()):
                                self.clear_events()
            except Exception:
                self.clear_events()
                get_logger(0).exception("Unexpected error in the HID loop")
                time.sleep(1)

    def __hid_loop_wait_device(self) -> bool:
        logger = get_logger(0)
        logger.info("Initial HID reset and wait ...")
        self.__gpio.reset()
        # На самом деле SPI и Serial-девайсы не пропадают, просто резет и ожидание
        # логичнее всего делать именно здесь. Ну и на будущее, да
        for _ in range(10):
            if self.__phy.has_device():
                logger.info("HID found")
                return True
            if self.__stop_event.is_set():
                break
            time.sleep(1)
        logger.error("Missing HID")
        return False

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

    def __set_state_busy(self, busy: bool) -> None:
        self.__state_flags.update(busy=int(busy))

    def __set_state_pong(self, response: bytes) -> None:
        status = response[1] << 16
        if len(response) > 4:
            status |= (response[2] << 8) | response[3]
        reset_required = (1 if response[1] & 0b01000000 else 0)
        self.__state_flags.update(online=1, busy=reset_required, status=status)
        if reset_required:
            self.__reset_required_event.set()
