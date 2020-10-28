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
import dataclasses
import contextlib
import queue
import struct
import time

from typing import Tuple
from typing import List
from typing import Dict
from typing import Iterable
from typing import Generator
from typing import AsyncGenerator

from ....logging import get_logger

from ....keyboard.mappings import KEYMAP

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


# =====
class _RequestError(Exception):
    def __init__(self, msg: str, online: bool=False) -> None:
        super().__init__(msg)
        self.msg = msg
        self.online = online


class _PermRequestError(_RequestError):
    pass


class _TempRequestError(_RequestError):
    pass


# =====
class _BaseEvent:
    def make_command(self) -> bytes:
        raise NotImplementedError


class _ClearEvent(_BaseEvent):
    def make_command(self) -> bytes:
        return b"\x10\x00\x00\x00\x00"


@dataclasses.dataclass(frozen=True)
class _KeyEvent(_BaseEvent):
    name: str
    state: bool

    def __post_init__(self) -> None:
        assert self.name in KEYMAP

    def make_command(self) -> bytes:
        code = KEYMAP[self.name].mcu.code
        return struct.pack(">BBBxx", 0x11, code, int(self.state))


@dataclasses.dataclass(frozen=True)
class _MouseButtonEvent(_BaseEvent):
    name: str
    state: bool

    def __post_init__(self) -> None:
        assert self.name in ["left", "right", "middle", "up", "down"]

    def make_command(self) -> bytes:
        (code, state_pressed, is_main) = {
            "left":   (0b10000000, 0b00001000, True),
            "right":  (0b01000000, 0b00000100, True),
            "middle": (0b00100000, 0b00000010, True),
            "up":     (0b10000000, 0b00001000, False),  # Back
            "down":   (0b01000000, 0b00000100, False),  # Forward
        }[self.name]
        if self.state:
            code |= state_pressed
        if is_main:
            main_code = code
            extra_code = 0
        else:
            main_code = 0
            extra_code = code
        return struct.pack(">BBBxx", 0x13, main_code, extra_code)


@dataclasses.dataclass(frozen=True)
class _MouseMoveEvent(_BaseEvent):
    to_x: int
    to_y: int

    def __post_init__(self) -> None:
        assert -32768 <= self.to_x <= 32767
        assert -32768 <= self.to_y <= 32767

    def make_command(self) -> bytes:
        return struct.pack(">Bhh", 0x12, self.to_x, self.to_y)


@dataclasses.dataclass(frozen=True)
class _MouseWheelEvent(_BaseEvent):
    delta_x: int
    delta_y: int

    def __post_init__(self) -> None:
        assert -127 <= self.delta_x <= 127
        assert -127 <= self.delta_y <= 127

    def make_command(self) -> bytes:
        # Горизонтальная прокрутка пока не поддерживается
        return struct.pack(">Bxbxx", 0x14, self.delta_y)


# =====
class BasePhyConnection:
    def send(self, request: bytes, receive: int) -> bytes:
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
        self.__gpio = Gpio(reset_pin, reset_delay)

        self.__events_queue: "multiprocessing.Queue[_BaseEvent]" = multiprocessing.Queue()

        self.__notifier = aiomulti.AioProcessNotifier()
        self.__state_flags = aiomulti.AioSharedFlags({
            "online": True,
            "caps": False,
            "scroll": False,
            "num": False,
        }, self.__notifier)

        self.__stop_event = multiprocessing.Event()

    @classmethod
    def get_plugin_options(cls) -> Dict:
        return {
            "reset_pin":   Option(-1,  type=valid_gpio_pin_optional),
            "reset_delay": Option(0.1, type=valid_float_f01),

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
        return {
            "online": state["online"],
            "keyboard": {
                "online": state["online"],
                "leds": {
                    "caps": state["caps"],
                    "scroll": state["scroll"],
                    "num": state["num"],
                },
            },
            "mouse": {"online": state["online"]},
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
                        self.__process_command(conn, b"\x10\x00\x00\x00\x00")
                except Exception:
                    logger.exception("Can't clear HID events")
        finally:
            self.__gpio.close()

    # =====

    def send_key_events(self, keys: Iterable[Tuple[str, bool]]) -> None:
        for (key, state) in keys:
            self.__queue_event(_KeyEvent(key, state))

    def send_mouse_button_event(self, button: str, state: bool) -> None:
        self.__queue_event(_MouseButtonEvent(button, state))

    def send_mouse_move_event(self, to_x: int, to_y: int) -> None:
        self.__queue_event(_MouseMoveEvent(to_x, to_y))

    def send_mouse_wheel_event(self, delta_x: int, delta_y: int) -> None:
        self.__queue_event(_MouseWheelEvent(delta_x, delta_y))

    def clear_events(self) -> None:
        # FIXME: Если очистка производится со стороны процесса хида, то возможна гонка между
        # очисткой и добавлением события _ClearEvent. Неприятно, но не смертельно.
        # Починить блокировкой после перехода на асинхронные очереди.
        tools.clear_queue(self.__events_queue)
        self.__queue_event(_ClearEvent())

    def __queue_event(self, event: _BaseEvent) -> None:
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
                                self.__process_command(conn, b"\x01\x00\x00\x00\x00")  # Ping
                            else:
                                if not self.__process_command(conn, event.make_command()):
                                    self.clear_events()
                else:
                    logger.error("Missing HID device")
                    time.sleep(1)
            except Exception:
                self.clear_events()
                logger.exception("Unexpected HID error")
                time.sleep(1)

    def __process_command(self, conn: BasePhyConnection, command: bytes) -> bool:
        return self.__process_request(conn, self.__make_request(command))

    def __process_request(self, conn: BasePhyConnection, request: bytes) -> bool:  # pylint: disable=too-many-branches
        logger = get_logger()
        error_messages: List[str] = []
        live_log_errors = False

        common_retries = self.__common_retries
        read_retries = self.__read_retries
        error_retval = False

        while common_retries and read_retries:
            response = self.__send_request(conn, request)
            try:
                if len(response) < 4:
                    read_retries -= 1
                    raise _TempRequestError(f"No response from HID: request={request!r}")

                assert len(response) == 4, response
                if self.__make_crc16(response[-4:-2]) != struct.unpack(">H", response[-2:])[0]:
                    request = self.__make_request(b"\x02\x00\x00\x00\x00")  # Repeat an answer
                    raise _TempRequestError("Invalid response CRC; requesting response again ...")

                code = response[1]
                if code == 0x48:  # Request timeout  # pylint: disable=no-else-raise
                    raise _TempRequestError(f"Got request timeout from HID: request={request!r}")
                elif code == 0x40:  # CRC Error
                    raise _TempRequestError(f"Got CRC error of request from HID: request={request!r}")
                elif code == 0x45:  # Unknown command
                    raise _PermRequestError(f"HID did not recognize the request={request!r}", online=True)
                elif code == 0x24:  # Rebooted?
                    raise _PermRequestError("No previous command state inside HID, seems it was rebooted", online=True)
                elif code == 0x20:  # Done
                    self.__state_flags.update(online=True)
                    return True
                elif code & 0x80:  # Pong with leds
                    self.__state_flags.update(
                        online=True,
                        caps=bool(code & 0b00000001),
                        scroll=bool(code & 0x00000010),
                        num=bool(code & 0x00000100),
                    )
                    return True
                raise _TempRequestError(f"Invalid response from HID: request={request!r}; code=0x{code:02X}")

            except _RequestError as err:
                common_retries -= 1
                self.__state_flags.update(online=err.online)
                error_retval = err.online

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
                    break
                if common_retries and read_retries:
                    time.sleep(self.__retries_delay)

        for msg in error_messages:
            logger.error(msg)
        if not (common_retries and read_retries):
            logger.error("Can't process HID request due many errors: %r", request)
        return error_retval

    def __send_request(self, conn: BasePhyConnection, request: bytes) -> bytes:
        if not self.__noop:
            response = conn.send(request, 4)
        else:
            response = b"\x33\x20"  # Magic + OK
            response += struct.pack(">H", self.__make_crc16(response))
        return response

    def __make_request(self, command: bytes) -> bytes:
        request = b"\x33" + command
        request += struct.pack(">H", self.__make_crc16(request))
        assert len(request) == 8, (request, command)
        return request

    def __make_crc16(self, data: bytes) -> int:
        crc = 0xFFFF
        for byte in data:
            crc = crc ^ byte
            for _ in range(8):
                if crc & 0x0001 == 0:
                    crc = crc >> 1
                else:
                    crc = crc >> 1
                    crc = crc ^ 0xA001
        return crc
