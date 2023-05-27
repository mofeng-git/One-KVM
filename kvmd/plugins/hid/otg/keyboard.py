# ========================================================================== #
#                                                                            #
#    KVMD - The main PiKVM daemon.                                           #
#                                                                            #
#    Copyright (C) 2018-2023  Maxim Devaev <mdevaev@gmail.com>               #
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


from typing import Iterable
from typing import Generator
from typing import Any

from ....logging import get_logger

from ....keyboard.mappings import UsbKey

from .device import BaseDeviceProcess

from .events import BaseEvent
from .events import ClearEvent
from .events import ResetEvent
from .events import KeyEvent
from .events import ModifierEvent
from .events import make_keyboard_event
from .events import get_led_caps
from .events import get_led_scroll
from .events import get_led_num
from .events import make_keyboard_report


# =====
class KeyboardProcess(BaseDeviceProcess):
    def __init__(self, **kwargs: Any) -> None:
        super().__init__(
            name="keyboard",
            read_size=1,
            initial_state={"caps": False, "scroll": False, "num": False},
            **kwargs,
        )

        self.__pressed_modifiers: set[UsbKey] = set()
        self.__pressed_keys: list[UsbKey | None] = [None] * 6

    def cleanup(self) -> None:
        self._stop()
        get_logger().info("Clearing HID-keyboard events ...")
        self._cleanup_write(b"\x00" * 8)  # Release all keys and modifiers

    def send_clear_event(self) -> None:
        self._clear_queue()
        self._queue_event(ClearEvent())

    def send_reset_event(self) -> None:
        self._clear_queue()
        self._queue_event(ResetEvent())

    def send_key_events(self, keys: Iterable[tuple[str, bool]]) -> None:
        for (key, state) in keys:
            self._queue_event(make_keyboard_event(key, state))

    # =====

    def _process_read_report(self, report: bytes) -> None:
        assert len(report) == 1, report
        self._update_state(
            caps=get_led_caps(report[0]),
            scroll=get_led_scroll(report[0]),
            num=get_led_num(report[0]),
        )

    # =====

    def _process_event(self, event: BaseEvent) -> Generator[bytes, None, None]:
        if isinstance(event, (ClearEvent, ResetEvent)):
            yield self.__process_clear_event()
        elif isinstance(event, ModifierEvent):
            yield from self.__process_modifier_event(event)
        elif isinstance(event, KeyEvent):
            yield from self.__process_key_event(event)
        else:
            raise RuntimeError(f"Not implemented event: {event}")

    def __process_clear_event(self) -> bytes:
        self.__clear_modifiers()
        self.__clear_keys()
        return self.__make_report()

    def __process_modifier_event(self, event: ModifierEvent) -> Generator[bytes, None, None]:
        if event.modifier in self.__pressed_modifiers:
            # Ранее нажатый модификатор отжимаем
            self.__pressed_modifiers.remove(event.modifier)
            yield self.__make_report()
        if event.state:
            # Нажимаем если нужно
            self.__pressed_modifiers.add(event.modifier)
            yield self.__make_report()

    def __process_key_event(self, event: KeyEvent) -> Generator[bytes, None, None]:
        if event.key in self.__pressed_keys:
            # Ранее нажатую клавишу отжимаем
            self.__pressed_keys[self.__pressed_keys.index(event.key)] = None
            yield self.__make_report()
        elif event.state and None not in self.__pressed_keys:
            # Если нужно нажать что-то новое, но свободных слотов нет - отжимаем всё
            self.__clear_keys()
            yield self.__make_report()
        if event.state:
            # Нажимаем если нужно
            self.__pressed_keys[self.__pressed_keys.index(None)] = event.key
            yield self.__make_report()

    # =====

    def __make_report(self) -> bytes:
        return make_keyboard_report(self.__pressed_modifiers, self.__pressed_keys)

    def __clear_modifiers(self) -> None:
        self.__pressed_modifiers.clear()

    def __clear_keys(self) -> None:
        self.__pressed_keys = [None] * 6
