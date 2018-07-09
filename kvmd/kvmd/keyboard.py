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
    # https://techdocs.altium.com/display/FPGA/PS2+Keyboard+Scan+Codes
    # http://www.vetra.com/scancodes.html

    get_logger().info(str(event))

    if event.key == "PrintScreen":
        return ([0xE0, 0x12, 0xE0, 0x7C] if event.state else [0xE0, 0xF0, 0x7C, 0xE0, 0xF0, 0x12])
    # TODO: pause/break
    else:
        codes = {
            "Escape":    [0x76],       "Backspace":  [0x66],
            "Tab":       [0x0D],       "Enter":      [0x5A],
            "Insert":    [0xE0, 0x70], "Delete":     [0xE0, 0x71],
            "Home":      [0xE0, 0x6C], "End":        [0xE0, 0x69],
            "PageUp":    [0xE0, 0x7D], "PageDown":   [0xE0, 0x7A],
            "ArrowLeft": [0xE0, 0x6B], "ArrowRight": [0xE0, 0x74],
            "ArrowUp":   [0xE0, 0x75], "ArrowDown":  [0xE0, 0x72],

            "CapsLock":    [0x58],
            "ScrollLock":  [0x7E],       "NumLock":      [0x77],
            "ShiftLeft":   [0x12],       "ShiftRight":   [0x59],
            "ControlLeft": [0x14],       "ControlRight": [0xE0, 0x14],
            "AltLeft":     [0x11],       "AltRight":     [0xE0, 0x11],
            "MetaLeft":    [0xE0, 0x1F], "MetaRight":    [0xE0, 0x27],

            "Backquote":   [0x0E], "Minus":        [0x4E], "Equal":     [0x55], "Space":     [0x29],
            "BracketLeft": [0x54], "BracketRight": [0x5B], "Semicolon": [0x4C], "Quote":     [0x52],
            "Comma":       [0x41], "Period":       [0x49], "Slash":     [0x4A], "Backslash": [0x5D],

            "Digit1": [0x16], "Digit2": [0x1E], "Digit3": [0x26], "Digit4": [0x25], "Digit5": [0x2E],
            "Digit6": [0x36], "Digit7": [0x3D], "Digit8": [0x3E], "Digit9": [0x46], "Digit0": [0x45],

            "KeyQ": [0x15], "KeyW": [0x1D], "KeyE": [0x24], "KeyR": [0x2D], "KeyT": [0x2C],
            "KeyY": [0x35], "KeyU": [0x3C], "KeyI": [0x43], "KeyO": [0x44], "KeyP": [0x4D],
            "KeyA": [0x1C], "KeyS": [0x1B], "KeyD": [0x23], "KeyF": [0x2B], "KeyG": [0x34],
            "KeyH": [0x33], "KeyJ": [0x3B], "KeyK": [0x42], "KeyL": [0x4B], "KeyZ": [0x1A],
            "KeyX": [0x22], "KeyC": [0x21], "KeyV": [0x2A], "KeyB": [0x32], "KeyN": [0x31],
            "KeyM": [0x3A],

            "F1": [0x05], "F2":  [0x06], "F3":  [0x04], "F4":  [0x0C],
            "F5": [0x03], "F6":  [0x0B], "F7":  [0x83], "F8":  [0x0A],
            "F9": [0x01], "F10": [0x09], "F11": [0x78], "F12": [0x07],

            # TODO: keypad
        }.get(event.key, [])
        if codes:
            if not event.state:
                assert 1 <= len(codes) <= 2, (event, codes)
                if len(codes) == 1:
                    codes = [0xF0, codes[0]]
                elif len(codes) == 2:
                    codes = [codes[0], 0xF0, codes[1]]
            return codes
    return []


class Keyboard(multiprocessing.Process):
    # http://dkudrow.blogspot.com/2013/08/ps2-keyboard-emulation-with-arduino-uno.html

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
