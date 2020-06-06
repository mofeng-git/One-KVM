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


import dataclasses

from typing import Tuple
from typing import List
from typing import Set
from typing import Iterable
from typing import Optional
from typing import Any

from ....logging import get_logger

from ....keyboard.mappings import OtgKey
from ....keyboard.mappings import KEYMAP

from .device import BaseEvent
from .device import BaseDeviceProcess


# =====
class _ClearEvent(BaseEvent):
    pass


class _ResetEvent(BaseEvent):
    pass


@dataclasses.dataclass(frozen=True)
class _ModifierEvent(BaseEvent):
    modifier: OtgKey
    state: bool

    def __post_init__(self) -> None:
        assert self.modifier.is_modifier


@dataclasses.dataclass(frozen=True)
class _KeyEvent(BaseEvent):
    key: OtgKey
    state: bool

    def __post_init__(self) -> None:
        assert not self.key.is_modifier


# =====
class KeyboardProcess(BaseDeviceProcess):
    def __init__(self, **kwargs: Any) -> None:
        super().__init__(
            name="keyboard",
            read_size=1,
            initial_state={"caps": False, "scroll": False, "num": False},
            **kwargs,
        )

        self.__pressed_modifiers: Set[OtgKey] = set()
        self.__pressed_keys: List[Optional[OtgKey]] = [None] * 6

    def cleanup(self) -> None:
        self._stop()
        get_logger().info("Clearing HID-keyboard events ...")
        self._ensure_write(b"\x00" * 8, close=True)  # Release all keys and modifiers

    def send_clear_event(self) -> None:
        self._clear_queue()
        self._queue_event(_ClearEvent())

    def send_reset_event(self) -> None:
        self._clear_queue()
        self._queue_event(_ResetEvent())

    def send_key_events(self, keys: Iterable[Tuple[str, bool]]) -> None:
        for (key, state) in keys:
            otg_key = KEYMAP[key].otg
            if otg_key.is_modifier:
                self._queue_event(_ModifierEvent(otg_key, state))
            else:
                self._queue_event(_KeyEvent(otg_key, state))

    # =====

    def _process_read_report(self, report: bytes) -> None:
        # https://wiki.osdev.org/USB_Human_Interface_Devices#LED_lamps
        assert len(report) == 1, report
        self._update_state(
            caps=bool(report[0] & 2),
            scroll=bool(report[0] & 4),
            num=bool(report[0] & 1),
        )

    # =====

    def _process_event(self, event: BaseEvent) -> None:
        if isinstance(event, _ClearEvent):
            self.__process_clear_event()
        elif isinstance(event, _ResetEvent):
            self.__process_clear_event(reopen=True)
        elif isinstance(event, _ModifierEvent):
            self.__process_modifier_event(event)
        elif isinstance(event, _KeyEvent):
            self.__process_key_event(event)

    def __process_clear_event(self, reopen: bool=False) -> None:
        self.__clear_modifiers()
        self.__clear_keys()
        self.__send_current_state(reopen=reopen)

    def __process_modifier_event(self, event: _ModifierEvent) -> None:
        if event.modifier in self.__pressed_modifiers:
            # Ранее нажатый модификатор отжимаем
            self.__pressed_modifiers.remove(event.modifier)
            if not self.__send_current_state():
                return
        if event.state:
            # Нажимаем если нужно
            self.__pressed_modifiers.add(event.modifier)
            self.__send_current_state()

    def __process_key_event(self, event: _KeyEvent) -> None:
        if event.key in self.__pressed_keys:
            # Ранее нажатую клавишу отжимаем
            self.__pressed_keys[self.__pressed_keys.index(event.key)] = None
            if not self.__send_current_state():
                return
        elif event.state and None not in self.__pressed_keys:
            # Если нужно нажать что-то новое, но свободных слотов нет - отжимаем всё
            self.__clear_keys()
            if not self.__send_current_state():
                return
        if event.state:
            # Нажимаем если нужно
            self.__pressed_keys[self.__pressed_keys.index(None)] = event.key
            self.__send_current_state()

    # =====

    def __send_current_state(self, reopen: bool=False) -> bool:
        if not self._ensure_write(self.__make_report(), reopen=reopen):
            self.__clear_modifiers()
            self.__clear_keys()
            return False
        return True

    def __clear_modifiers(self) -> None:
        self.__pressed_modifiers.clear()

    def __clear_keys(self) -> None:
        self.__pressed_keys = [None] * 6

    def __make_report(self) -> bytes:
        modifiers = 0
        for modifier in self.__pressed_modifiers:
            modifiers |= modifier.code

        assert len(self.__pressed_keys) == 6
        keys = [
            (0 if key is None else key.code)
            for key in self.__pressed_keys
        ]

        return bytes([modifiers, 0] + keys)
