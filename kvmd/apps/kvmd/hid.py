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
import signal
import asyncio
import multiprocessing
import multiprocessing.queues
import queue
import struct
import pkgutil
import time

from typing import Dict
from typing import Set
from typing import NamedTuple
from typing import AsyncGenerator
from typing import Any

import yaml
import serial
import setproctitle

from ...logging import get_logger

from ... import gpio


# =====
def _get_keymap() -> Dict[str, int]:
    return yaml.safe_load(pkgutil.get_data("kvmd", "data/keymap.yaml").decode())  # type: ignore


_KEYMAP = _get_keymap()


class _KeyEvent(NamedTuple):
    key: str
    state: bool

    @staticmethod
    def is_valid(key: str) -> bool:
        return (key in _KEYMAP)

    def make_command(self) -> bytes:
        code = _KEYMAP[self.key]
        key_bytes = bytes([code])
        assert len(key_bytes) == 1, (self, key_bytes, code)
        state_bytes = (b"\x01" if self.state else b"\x00")
        return b"\x11" + key_bytes + state_bytes + b"\x00\x00"


class _MouseMoveEvent(NamedTuple):
    to_x: int
    to_y: int

    def make_command(self) -> bytes:
        to_x = min(max(-32768, self.to_x), 32767)
        to_y = min(max(-32768, self.to_y), 32767)
        return b"\x12" + struct.pack(">hh", to_x, to_y)


class _MouseButtonEvent(NamedTuple):
    button: str
    state: bool

    @staticmethod
    def is_valid(button: str) -> bool:
        return (button in ["left", "right"])

    def make_command(self) -> bytes:
        code = 0
        if self.button == "left":
            code = (0b10000000 | (0b00001000 if self.state else 0))
        elif self.button == "right":
            code = (0b01000000 | (0b00000100 if self.state else 0))
        assert code, self
        return b"\x13" + bytes([code]) + b"\x00\x00\x00"


class _MouseWheelEvent(NamedTuple):
    delta_y: int

    def make_command(self) -> bytes:
        delta_y = min(max(-128, self.delta_y), 127)
        return b"\x14\x00" + struct.pack(">b", delta_y) + b"\x00\x00"


# =====
class Hid(multiprocessing.Process):  # pylint: disable=too-many-instance-attributes
    def __init__(  # pylint: disable=too-many-arguments
        self,
        reset_pin: int,
        reset_delay: float,

        device_path: str,
        speed: int,
        read_timeout: float,
        read_retries: int,
        common_retries: int,
        retries_delay: float,
        noop: bool,

        state_poll: float,
    ) -> None:

        super().__init__(daemon=True)

        self.__reset_pin = gpio.set_output(reset_pin)
        self.__reset_delay = reset_delay

        self.__device_path = device_path
        self.__speed = speed
        self.__read_timeout = read_timeout
        self.__read_retries = read_retries
        self.__common_retries = common_retries
        self.__retries_delay = retries_delay
        self.__noop = noop

        self.__state_poll = state_poll

        self.__pressed_keys: Set[str] = set()
        self.__pressed_mouse_buttons: Set[str] = set()
        self.__lock = asyncio.Lock()
        self.__events_queue: multiprocessing.queues.Queue = multiprocessing.Queue()

        self.__online_shared = multiprocessing.Value("i", 1)

        self.__stop_event = multiprocessing.Event()

    def start(self) -> None:
        get_logger().info("Starting HID daemon ...")
        super().start()

    def get_state(self) -> Dict:
        return {"online": bool(self.__online_shared.value)}

    async def poll_state(self) -> AsyncGenerator[Dict, None]:
        while self.is_alive():
            yield self.get_state()
            await asyncio.sleep(self.__state_poll)

    async def reset(self) -> None:
        async with self.__lock:
            gpio.write(self.__reset_pin, True)
            await asyncio.sleep(self.__reset_delay)
            gpio.write(self.__reset_pin, False)

    async def send_key_event(self, key: str, state: bool) -> None:
        await self.__send_bool_event(_KeyEvent, self.__pressed_keys, key, state)

    async def send_mouse_move_event(self, to_x: int, to_y: int) -> None:
        await self.__send_int_event(_MouseMoveEvent, to_x, to_y)

    async def send_mouse_button_event(self, button: str, state: bool) -> None:
        await self.__send_bool_event(_MouseButtonEvent, self.__pressed_mouse_buttons, button, state)

    async def send_mouse_wheel_event(self, delta_y: int) -> None:
        await self.__send_int_event(_MouseWheelEvent, delta_y)

    async def clear_events(self) -> None:
        if not self.__stop_event.is_set():
            async with self.__lock:
                self.__unsafe_clear_events()

    async def cleanup(self) -> None:
        async with self.__lock:
            if self.is_alive():
                self.__unsafe_clear_events()
                get_logger().info("Stopping HID daemon ...")
                self.__stop_event.set()
                self.join()
            else:
                get_logger().warning("Emergency cleaning up HID events ...")
                self.__emergency_clear_events()
            gpio.write(self.__reset_pin, False)

    async def __send_bool_event(self, cls: Any, pressed: Set[str], name: str, state: bool) -> None:
        if not self.__stop_event.is_set():
            async with self.__lock:
                if cls.is_valid(name) and (
                    (state and (name not in pressed))  # Если еще не нажато
                    or (not state and (name in pressed))  # ... Или еще не отжато
                ):
                    if state:
                        pressed.add(name)
                    else:
                        pressed.remove(name)
                    self.__events_queue.put(cls(name, state))

    async def __send_int_event(self, cls: Any, *args: int) -> None:
        if not self.__stop_event.is_set():
            async with self.__lock:
                self.__events_queue.put(cls(*args))

    def __unsafe_clear_events(self) -> None:
        for (cls, pressed) in [
            (_MouseButtonEvent, self.__pressed_mouse_buttons),
            (_KeyEvent, self.__pressed_keys),
        ]:
            for name in pressed:
                self.__events_queue.put(cls(name, False))
            pressed.clear()

    def __emergency_clear_events(self) -> None:
        if os.path.exists(self.__device_path):
            try:
                with self.__get_serial() as tty:
                    self.__process_command(tty, b"\x10\x00\x00\x00\x00")
            except Exception:
                get_logger().exception("Can't execute emergency clear HID events")

    def run(self) -> None:  # pylint: disable=too-many-branches
        signal.signal(signal.SIGINT, signal.SIG_IGN)
        setproctitle.setproctitle("[hid] " + setproctitle.getproctitle())
        try:
            with self.__get_serial() as tty:
                passed = 0
                while not (self.__stop_event.is_set() and self.__events_queue.qsize() == 0):
                    try:
                        event = self.__events_queue.get(timeout=0.05)
                    except queue.Empty:
                        if passed >= 20:  # 20 * 0.05 = 1 sec
                            self.__process_command(tty, b"\x01\x00\x00\x00\x00")  # Ping
                            passed = 0
                        else:
                            passed += 1
                    else:
                        self.__process_command(tty, event.make_command())
                        passed = 0
        except Exception:
            get_logger().exception("Unhandled exception")
            raise

    def __get_serial(self) -> serial.Serial:
        return serial.Serial(self.__device_path, self.__speed, timeout=self.__read_timeout)

    def __process_command(self, tty: serial.Serial, command: bytes) -> None:
        self.__process_request(tty, self.__make_request(command))

    def __process_request(self, tty: serial.Serial, request: bytes) -> None:  # pylint: disable=too-many-branches
        logger = get_logger()

        common_retries = self.__common_retries
        read_retries = self.__read_retries
        error_occured = False

        while common_retries and read_retries:
            if not self.__noop:
                if tty.in_waiting:
                    tty.read(tty.in_waiting)

                assert tty.write(request) == len(request)
                response = tty.read(4)
            else:
                response = b"\x33\x20"  # Magic + OK
                response += struct.pack(">H", self.__make_crc16(response))

            if len(response) < 4:
                logger.error("No response from HID: request=%r", request)
                read_retries -= 1
            else:
                assert len(response) == 4, response
                if self.__make_crc16(response[-4:-2]) != struct.unpack(">H", response[-2:])[0]:
                    get_logger().error("Invalid response CRC; requesting response again ...")
                    request = self.__make_request(b"\x02\x00\x00\x00\x00")  # Repeat an answer
                else:
                    code = response[1]
                    if code == 0x48:  # Request timeout
                        logger.error("Got request timeout from HID: request=%r", request)
                    elif code == 0x40:  # CRC Error
                        logger.error("Got CRC error of request from HID: request=%r", request)
                    elif code == 0x45:  # Unknown command
                        logger.error("HID did not recognize the request=%r", request)
                        self.__online_shared.value = 1
                        return
                    elif code == 0x24:  # Rebooted?
                        logger.error("No previous command state inside HID, seems it was rebooted")
                        self.__online_shared.value = 1
                        return
                    elif code == 0x20:  # Done
                        if error_occured:
                            logger.info("Success!")
                        self.__online_shared.value = 1
                        return
                    else:
                        logger.error("Invalid response from HID: request=%r; code=0x%x", request, code)

                common_retries -= 1
            error_occured = True
            self.__online_shared.value = 0

            if common_retries and read_retries:
                logger.error("Retries left: common_retries=%d; read_retries=%d", common_retries, read_retries)
                time.sleep(self.__retries_delay)

        logger.error("Can't process HID request due many errors: %r", request)

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
