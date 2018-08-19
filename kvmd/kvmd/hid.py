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

import yaml
import serial

from .logging import get_logger


# =====
def _get_keymap() -> Dict[str, int]:
    return yaml.load(pkgutil.get_data(__name__, "data/keymap.yaml").decode())  # type: ignore


_KEYMAP = _get_keymap()


class _KeyEvent(NamedTuple):
    key: str
    state: bool


class _MouseMoveEvent(NamedTuple):
    to_x: int
    to_y: int


class _MouseButtonEvent(NamedTuple):
    button: str
    state: bool


class _MouseWheelEvent(NamedTuple):
    delta_y: int


class Hid(multiprocessing.Process):
    def __init__(
        self,
        device_path: str,
        speed: int,
    ) -> None:

        super().__init__(daemon=True)

        self.__device_path = device_path
        self.__speed = speed

        self.__pressed_keys: Set[str] = set()
        self.__pressed_mouse_buttons: Set[str] = set()
        self.__lock = asyncio.Lock()
        self.__queue: multiprocessing.queues.Queue = multiprocessing.Queue()

        self.__stop_event = multiprocessing.Event()

    def start(self) -> None:
        get_logger().info("Starting HID daemon ...")
        super().start()

    # TODO: add reset or power switching

    async def send_key_event(self, key: str, state: bool) -> None:
        if not self.__stop_event.is_set():
            async with self.__lock:
                if state and key not in self.__pressed_keys:
                    self.__pressed_keys.add(key)
                    self.__queue.put(_KeyEvent(key, state))
                elif not state and key in self.__pressed_keys:
                    self.__pressed_keys.remove(key)
                    self.__queue.put(_KeyEvent(key, state))

    async def send_mouse_move_event(self, to_x: int, to_y: int) -> None:
        if not self.__stop_event.is_set():
            async with self.__lock:
                self.__queue.put(_MouseMoveEvent(to_x, to_y))

    async def send_mouse_button_event(self, button: str, state: bool) -> None:
        if not self.__stop_event.is_set():
            async with self.__lock:
                if state and button not in self.__pressed_mouse_buttons:
                    self.__pressed_mouse_buttons.add(button)
                    self.__queue.put(_MouseButtonEvent(button, state))
                elif not state and button in self.__pressed_mouse_buttons:
                    self.__pressed_mouse_buttons.remove(button)
                    self.__queue.put(_MouseButtonEvent(button, state))

    async def send_mouse_wheel_event(self, delta_y: int) -> None:
        if not self.__stop_event.is_set():
            async with self.__lock:
                self.__queue.put(_MouseWheelEvent(delta_y))

    async def clear_events(self) -> None:
        if not self.__stop_event.is_set():
            async with self.__lock:
                self.__unsafe_clear_events()

    async def cleanup(self) -> None:
        async with self.__lock:
            if self.is_alive():
                self.__unsafe_clear_events()
                get_logger().info("Stopping keyboard daemon ...")
                self.__stop_event.set()
                self.join()
            else:
                get_logger().warning("Emergency cleaning up HID events ...")
                self.__emergency_clear_events()

    def __unsafe_clear_events(self) -> None:
        for button in self.__pressed_mouse_buttons:
            self.__queue.put(_MouseButtonEvent(button, False))
        self.__pressed_mouse_buttons.clear()
        for key in self.__pressed_keys:
            self.__queue.put(_KeyEvent(key, False))
        self.__pressed_keys.clear()

    def __emergency_clear_events(self) -> None:
        try:
            with serial.Serial(self.__device_path, self.__speed) as tty:
                self.__send_clear_hid(tty)
        except Exception:
            get_logger().exception("Can't execute emergency clear HID events")

    def run(self) -> None:  # pylint: disable=too-many-branches
        try:
            with serial.Serial(self.__device_path, self.__speed) as tty:
                hid_ready = False
                while True:
                    if hid_ready:
                        try:
                            event = self.__queue.get(timeout=0.05)
                        except queue.Empty:
                            pass
                        else:
                            if isinstance(event, _KeyEvent):
                                self.__send_key_event(tty, event)
                            elif isinstance(event, _MouseMoveEvent):
                                self.__send_mouse_move_event(tty, event)
                            elif isinstance(event, _MouseButtonEvent):
                                self.__send_mouse_button_event(tty, event)
                            elif isinstance(event, _MouseWheelEvent):
                                self.__send_mouse_wheel_event(tty, event)
                            else:
                                raise RuntimeError("Unknown HID event")
                            hid_ready = False
                    else:
                        if tty.in_waiting:
                            while tty.in_waiting:
                                tty.read(tty.in_waiting)
                            hid_ready = True
                        else:
                            time.sleep(0.05)
                    if self.__stop_event.is_set() and self.__queue.qsize() == 0:
                        break
        except Exception:
            get_logger().exception("Unhandled exception")
            raise

    def __send_key_event(self, tty: serial.Serial, event: _KeyEvent) -> None:
        code = _KEYMAP.get(event.key)
        if code:
            key_bytes = bytes([code])
            assert len(key_bytes) == 1, (event, key_bytes)
            tty.write(
                b"\01"
                + key_bytes
                + (b"\01" if event.state else b"\00")
                + b"\00\00"
            )

    def __send_mouse_move_event(self, tty: serial.Serial, event: _MouseMoveEvent) -> None:
        to_x = min(max(-32768, event.to_x), 32767)
        to_y = min(max(-32768, event.to_y), 32767)
        tty.write(b"\02" + struct.pack(">hh", to_x, to_y))

    def __send_mouse_button_event(self, tty: serial.Serial, event: _MouseButtonEvent) -> None:
        if event.button == "left":
            code = (0b10000000 | (0b00001000 if event.state else 0))
        elif event.button == "right":
            code = (0b01000000 | (0b00000100 if event.state else 0))
        else:
            code = 0
        if code:
            tty.write(b"\03" + bytes([code]) + b"\00\00\00")

    def __send_mouse_wheel_event(self, tty: serial.Serial, event: _MouseWheelEvent) -> None:
        delta_y = min(max(-128, event.delta_y), 127)
        tty.write(b"\04\00" + struct.pack(">b", delta_y) + b"\00\00")

    def __send_clear_hid(self, tty: serial.Serial) -> None:
        tty.write(b"\00\00\00\00\00")
