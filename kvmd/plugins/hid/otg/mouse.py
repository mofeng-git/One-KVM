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


import struct
import dataclasses

from typing import Any

from ....logging import get_logger

from .device import BaseEvent
from .device import BaseDeviceProcess


# =====
_BUTTONS = {
    "left":   0x1,
    "right":  0x2,
    "middle": 0x4,
}


# =====
class _ClearEvent(BaseEvent):
    pass


class _ResetEvent(BaseEvent):
    pass


@dataclasses.dataclass(frozen=True)
class _ButtonEvent(BaseEvent):
    code: int
    state: bool


@dataclasses.dataclass(frozen=True)
class _MoveEvent(BaseEvent):
    to_x: int
    to_y: int

    def __post_init__(self) -> None:
        assert -32768 <= self.to_x <= 32767
        assert -32768 <= self.to_y <= 32767


@dataclasses.dataclass(frozen=True)
class _WheelEvent(BaseEvent):
    delta_x: int
    delta_y: int

    def __post_init__(self) -> None:
        assert -127 <= self.delta_x <= 127
        assert -127 <= self.delta_y <= 127


# =====
class MouseProcess(BaseDeviceProcess):
    def __init__(self, **kwargs: Any) -> None:
        super().__init__(name="mouse", **kwargs)

        self.__pressed_buttons: int = 0
        self.__x = 0
        self.__y = 0

    def cleanup(self) -> None:
        self._stop()
        get_logger().info("Clearing HID-mouse events ...")
        if self._ensure_device():
            try:
                self._write_report(self.__make_report(0, self.__x, self.__y, 0, 0))  # Release all buttons
            finally:
                self._close_device()

    def send_clear_event(self) -> None:
        self._queue_event(_ClearEvent())

    def send_reset_event(self) -> None:
        self._queue_event(_ResetEvent())

    def send_button_event(self, button: str, state: bool) -> None:
        assert button in _BUTTONS
        self._queue_event(_ButtonEvent(code=_BUTTONS[button], state=state))

    def send_move_event(self, to_x: int, to_y: int) -> None:
        self._queue_event(_MoveEvent(to_x, to_y))

    def send_wheel_event(self, delta_x: int, delta_y: int) -> None:
        self._queue_event(_WheelEvent(delta_x, delta_y))

    # =====

    def _process_event(self, event: BaseEvent) -> None:
        if isinstance(event, _ClearEvent):
            self.__process_clear_event()
        elif isinstance(event, _ResetEvent):
            self.__process_clear_event(reopen=True)
        elif isinstance(event, _ButtonEvent):
            self.__process_button_event(event)
        elif isinstance(event, _MoveEvent):
            self.__process_move_event(event)
        elif isinstance(event, _WheelEvent):
            self.__process_wheel_event(event)

    def __process_clear_event(self, reopen: bool=False) -> None:
        self.__clear_state()
        if reopen:
            self._close_device()
        self.__send_current_state(0, 0)

    def __process_button_event(self, event: _ButtonEvent) -> None:
        if event.code & self.__pressed_buttons:
            # Ранее нажатую кнопку отжимаем
            self.__pressed_buttons &= ~event.code
            if not self.__send_current_state(0, 0):
                return
        if event.state:
            # Нажимаем если нужно
            self.__pressed_buttons |= event.code
            self.__send_current_state(0, 0)

    def __process_move_event(self, event: _MoveEvent) -> None:
        self.__x = event.to_x
        self.__y = event.to_y
        self.__send_current_state(0, 0)

    def __process_wheel_event(self, event: _WheelEvent) -> None:
        self.__send_current_state(event.delta_x, event.delta_y)

    def __send_current_state(self, delta_x: int, delta_y: int) -> bool:
        ok = False
        if self._ensure_device():
            ok = self._write_report(self.__make_report(
                buttons=self.__pressed_buttons,
                to_x=self.__x,
                to_y=self.__y,
                delta_x=delta_x,
                delta_y=delta_y,
            ))
        if not ok:
            self.__clear_state()
        return ok

    def __clear_state(self) -> None:
        self.__pressed_buttons = 0
        self.__x = 0
        self.__y = 0

    def __make_report(self, buttons: int, to_x: int, to_y: int, delta_x: int, delta_y: int) -> bytes:
        to_x = (to_x + 32768) // 2
        to_y = (to_y + 32768) // 2
        return struct.pack("<BHHbb", buttons, to_x, to_y, delta_y, delta_x)
