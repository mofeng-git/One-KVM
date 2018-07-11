import asyncio
import multiprocessing
import multiprocessing.queues
import queue
import pkgutil

from typing import Dict
from typing import Set
from typing import NamedTuple

import yaml
import serial

from .logging import get_logger

from . import gpio


# =====
def _get_keymap() -> Dict[str, int]:
    return yaml.load(pkgutil.get_data(__name__, "data/keymap.yaml").decode())  # type: ignore


_KEYMAP = _get_keymap()


def _keymap(key: str) -> bytes:
    code = _KEYMAP.get(key)
    return (bytes([code]) if code else b"")  # type: ignore


class _KeyEvent(NamedTuple):
    key: str
    state: bool


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
        key_bytes = _keymap(event.key)
        if key_bytes:
            assert len(key_bytes) == 1, (event, key_bytes)
            tty.write(
                b"\01"
                + (b"\01" if event.state else b"\00")
                + key_bytes
                + b"\00"
            )

    def __send_clear_hid(self, tty: serial.Serial) -> None:
        tty.write(b"\00\00\00\00")
