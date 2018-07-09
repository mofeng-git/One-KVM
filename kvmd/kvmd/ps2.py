import asyncio
import multiprocessing
import multiprocessing.queues
import queue
import time

from typing import List
from typing import Set
from typing import NamedTuple

from .logging import get_logger

from . import gpio


# =====
class _KeyEvent(NamedTuple):
    key: str
    state: bool


def _key_event_to_ps2_codes(event: _KeyEvent) -> List[int]:
    get_logger().info(str(event))
    return []  # TODO


class Ps2Keyboard(multiprocessing.Process):
    def __init__(self, clock: int, data: int, pulse: float) -> None:
        super().__init__(daemon=True)

        self.__clock = gpio.set_output(clock, initial=True)
        self.__data = gpio.set_output(data, initial=True)
        self.__pulse = pulse

        self.__pressed_keys: Set[str] = set()
        self.__lock = asyncio.Lock()
        self.__queue: multiprocessing.queues.Queue = multiprocessing.Queue()

        self.__stop_event = multiprocessing.Event()

    def start(self) -> None:
        get_logger().info("Starting keyboard daemon ...")
        super().start()

    async def send_event(self, key: str, state: bool) -> None:
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
        for key in self.__pressed_keys:
            for code in _key_event_to_ps2_codes(_KeyEvent(key, False)):
                self.__send_byte(code)

    def run(self) -> None:
        with gpio.bcm():
            try:
                while True:
                    try:
                        event = self.__queue.get(timeout=0.1)
                    except queue.Empty:
                        pass
                    else:
                        for code in _key_event_to_ps2_codes(event):
                            self.__send_byte(code)
                    if self.__stop_event.is_set() and self.__queue.qsize() == 0:
                        break
            except Exception:
                get_logger().exception("Unhandled exception")
                raise

    def __send_byte(self, code: int) -> None:
        code_bits = list(map(bool, bin(code)[2:].zfill(8)))
        code_bits.reverse()
        message = [False] + code_bits + [(not sum(code_bits) % 2), True]
        for bit in message:
            self.__send_bit(bit)

    def __send_bit(self, bit: bool) -> None:
        gpio.write(self.__clock, True)
        gpio.write(self.__data, bool(bit))
        time.sleep(self.__pulse)
        gpio.write(self.__clock, False)
        time.sleep(self.__pulse)
        gpio.write(self.__clock, True)
