import re
import asyncio
import multiprocessing
import multiprocessing.queues
import queue

from typing import Set
from typing import NamedTuple
from typing import Union

import serial

from .logging import get_logger

from . import gpio


# =====
class _KeyEvent(NamedTuple):
    key: str
    state: bool


def _key_to_bytes(key: str) -> bytes:
    # https://www.arduino.cc/reference/en/language/functions/usb/keyboard/
    # Also locate Keyboard.h

    match = re.match(r"(Digit|Key)([0-9A-Z])", key)
    code: Union[str, int, None]
    if match:
        code = match.group(2)
    else:
        code = {  # type: ignore
            "Escape":    0xB1, "Backspace":  0xB2,
            "Tab":       0xB3, "Enter":      0xB0,
            "Insert":    0xD1, "Delete":     0xD4,
            "Home":      0xD2, "End":        0xD5,
            "PageUp":    0xD3, "PageDown":   0xD6,
            "ArrowLeft": 0xD8, "ArrowRight": 0xD7,
            "ArrowUp":   0xDA, "ArrowDown":  0xD9,

            "CapsLock":    0xC1,
            "ShiftLeft":   0x81, "ShiftRight":   0x85,
            "ControlLeft": 0x80, "ControlRight": 0x84,
            "AltLeft":     0x82, "AltRight":     0x86,
            "MetaLeft":    0x83, "MetaRight":    0x87,

            "Backquote":   "`", "Minus":        "-", "Equal":     "=", "Space":     " ",
            "BracketLeft": "[", "BracketRight": "]", "Semicolon": ";", "Quote":     "'",
            "Comma":       ",", "Period":       ".", "Slash":     "/", "Backslash": "\\",

            "F1": 0xC2, "F2":  0xC3, "F3":  0xC4, "F4":  0xC5,
            "F5": 0xC6, "F6":  0xC7, "F7":  0xC8, "F8":  0xC9,
            "F9": 0xCA, "F10": 0xCB, "F11": 0xCC, "F12": 0xCD,
        }.get(key)
    if isinstance(code, str):
        return bytes(code, encoding="ascii")  # type: ignore
    elif isinstance(code, int):
        return bytes([code])
    return b""


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
        self.__lock = asyncio.Lock()
        self.__queue: multiprocessing.queues.Queue = multiprocessing.Queue()

        self.__stop_event = multiprocessing.Event()

    def start(self) -> None:
        get_logger().info("Starting HID daemon ...")
        super().start()

    async def send_key_event(self, key: str, state: bool) -> None:
        if not self.__stop_event.is_set():
            async with self.__lock:
                if state and key not in self.__pressed_keys:
                    self.__pressed_keys.add(key)
                    self.__queue.put(_KeyEvent(key, state))
                elif not state and key in self.__pressed_keys:
                    self.__pressed_keys.remove(key)
                    self.__queue.put(_KeyEvent(key, state))

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
                get_logger().warning("Emergency cleaning up keyboard events ...")
                self.__emergency_clear_events()

    def __unsafe_clear_events(self) -> None:
        for key in self.__pressed_keys:
            self.__queue.put(_KeyEvent(key, False))
        self.__pressed_keys.clear()

    def __emergency_clear_events(self) -> None:
        try:
            with serial.Serial(self.__device_path, self.__speed) as tty:
                self.__send_clear_hid(tty)
        except Exception:
            get_logger().exception("Can't execute emergency clear events")

    def run(self) -> None:
        with gpio.bcm():
            try:
                with serial.Serial(self.__device_path, self.__speed) as tty:
                    while True:
                        try:
                            event = self.__queue.get(timeout=0.1)
                        except queue.Empty:
                            pass
                        else:
                            self.__send_key_event(tty, event)
                        if self.__stop_event.is_set() and self.__queue.qsize() == 0:
                            break
            except Exception:
                get_logger().exception("Unhandled exception")
                raise

    def __send_key_event(self, tty: serial.Serial, event: _KeyEvent) -> None:
        key_bytes = _key_to_bytes(event.key)
        if key_bytes:
            assert len(key_bytes) == 1, (event, key_bytes)
            tty.write(
                b"\01"
                + (b"\01" if event.state else b"\00")
                + key_bytes
            )

    def __send_clear_hid(self, tty: serial.Serial) -> None:
        tty.write(b"\00")
