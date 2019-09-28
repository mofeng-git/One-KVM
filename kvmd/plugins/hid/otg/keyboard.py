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

from typing import List
from typing import Set
from typing import Optional
from typing import Any

from ....logging import get_logger

from .... import keymap

from .hid import BaseEvent
from .hid import DeviceProcess


# =====
class _ClearEvent(BaseEvent):
    pass


class _ResetEvent(BaseEvent):
    pass


@dataclasses.dataclass(frozen=True)
class _KeyEvent(BaseEvent):
    key: keymap.OtgKey
    state: bool


# =====
class KeyboardProcess(DeviceProcess):
    def __init__(self, **kwargs: Any) -> None:
        super().__init__(name="keyboard", **kwargs)

        self.__pressed_modifiers: Set[keymap.OtgKey] = set()
        self.__pressed_keys: List[Optional[keymap.OtgKey]] = [None] * 6

    def cleanup(self) -> None:
        self._stop()
        get_logger().info("Clearing HID-keyboard events ...")
        if self._ensure_device():
            try:
                self._write_report(b"\x00" * 8)  # Release all keys and modifiers
            finally:
                self._close_device()

    def send_clear_event(self) -> None:
        self._queue_event(_ClearEvent())

    def send_reset_event(self) -> None:
        self._queue_event(_ResetEvent())

    def send_key_event(self, key: str, state: bool) -> None:
        assert key in keymap.KEYMAP
        self._queue_event(_KeyEvent(key=keymap.KEYMAP[key].otg, state=state))

    # =====

    def _process_event(self, event: BaseEvent) -> None:
        if isinstance(event, _ClearEvent):
            self.__process_clear_event()
        elif isinstance(event, _ResetEvent):
            self.__process_clear_event(reopen=True)
        elif isinstance(event, _KeyEvent):
            self.__process_key_event(event)

    def __process_clear_event(self, reopen: bool=False) -> None:
        self.__clear_modifiers()
        self.__clear_keys()
        if reopen:
            self._close_device()
        self.__send_current_state()

    def __process_key_event(self, event: _KeyEvent) -> None:
        if event.key.is_modifier:
            if event.key in self.__pressed_modifiers:
                # Ранее нажатый модификатор отжимаем
                self.__pressed_modifiers.remove(event.key)
                if not self.__send_current_state():
                    return
            if event.state:
                # Нажимаем если нужно
                self.__pressed_modifiers.add(event.key)
                self.__send_current_state()

        else:  # regular key
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

    def __send_current_state(self) -> bool:
        ok = False
        if self._ensure_device():
            modifiers = 0
            for key in self.__pressed_modifiers:
                assert key.is_modifier
                modifiers |= key.code

            assert len(self.__pressed_keys) == 6
            keys = [
                (0 if key is None else key.code)
                for key in self.__pressed_keys
            ]

            print(self.__pressed_modifiers, self.__pressed_keys)
            ok = self._write_report(bytes([modifiers, 0] + keys))

        if not ok:
            self.__clear_modifiers()
            self.__clear_keys()
        return ok

    def __clear_modifiers(self) -> None:
        self.__pressed_modifiers.clear()

    def __clear_keys(self) -> None:
        self.__pressed_keys = [None] * 6
