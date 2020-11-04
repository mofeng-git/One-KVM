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

from typing import Optional
from typing import Any

from ....logging import get_logger

from .device import BaseDeviceProcess

from .events import BaseEvent
from .events import ClearEvent
from .events import ResetEvent
from .events import MouseButtonEvent
from .events import MouseMoveEvent
from .events import MouseRelativeEvent
from .events import MouseWheelEvent


# =====
class MouseProcess(BaseDeviceProcess):
    def __init__(self, **kwargs: Any) -> None:
        self.__absolute: bool = kwargs.pop("absolute")
        self.__horizontal_wheel: bool = kwargs.pop("horizontal_wheel")

        super().__init__(
            name="mouse",
            read_size=0,
            initial_state={"absolute": self.__absolute},  # Just for the state
            **kwargs,
        )

        self.__pressed_buttons: int = 0
        self.__x = 0  # For absolute
        self.__y = 0

    def cleanup(self) -> None:
        self._stop()
        get_logger().info("Clearing HID-mouse events ...")
        if self.__absolute:
            report = self.__make_report(0, self.__x, self.__y, 0, 0)
        else:
            report = self.__make_report(0, 0, 0, 0, 0)
        self._ensure_write(report, close=True)  # Release all buttons

    def send_clear_event(self) -> None:
        self._clear_queue()
        self._queue_event(ClearEvent())

    def send_reset_event(self) -> None:
        self._clear_queue()
        self._queue_event(ResetEvent())

    def send_button_event(self, button: str, state: bool) -> None:
        self._queue_event(MouseButtonEvent(button, state))

    def send_move_event(self, to_x: int, to_y: int) -> None:
        if self.__absolute:
            self._queue_event(MouseMoveEvent(to_x, to_y))

    def send_relative_event(self, delta_x: int, delta_y: int) -> None:
        if not self.__absolute:
            self._queue_event(MouseRelativeEvent(delta_x, delta_y))

    def send_wheel_event(self, delta_x: int, delta_y: int) -> None:
        self._queue_event(MouseWheelEvent(delta_x, delta_y))

    # =====

    def _process_event(self, event: BaseEvent) -> bool:
        if isinstance(event, ClearEvent):
            return self.__process_clear_event()
        elif isinstance(event, ResetEvent):
            return self.__process_clear_event(reopen=True)
        elif isinstance(event, MouseButtonEvent):
            return self.__process_button_event(event)
        elif isinstance(event, MouseMoveEvent):
            return self.__process_move_event(event)
        elif isinstance(event, MouseRelativeEvent):
            return self.__process_relative_event(event)
        elif isinstance(event, MouseWheelEvent):
            return self.__process_wheel_event(event)
        raise RuntimeError(f"Not implemented event: {event}")

    def __process_clear_event(self, reopen: bool=False) -> bool:
        self.__clear_state()
        return self.__send_current_state(reopen=reopen)

    def __process_button_event(self, event: MouseButtonEvent) -> bool:
        if event.code & self.__pressed_buttons:
            # Ранее нажатую кнопку отжимаем
            self.__pressed_buttons &= ~event.code
            if not self.__send_current_state():
                return False
        if event.state:
            # Нажимаем если нужно
            self.__pressed_buttons |= event.code
            return self.__send_current_state()
        return True

    def __process_move_event(self, event: MouseMoveEvent) -> bool:
        self.__x = event.to_fixed_x
        self.__y = event.to_fixed_y
        return self.__send_current_state()

    def __process_relative_event(self, event: MouseRelativeEvent) -> bool:
        return self.__send_current_state(relative_event=event)

    def __process_wheel_event(self, event: MouseWheelEvent) -> bool:
        return self.__send_current_state(wheel_event=event)

    # =====

    def __send_current_state(
        self,
        relative_event: Optional[MouseRelativeEvent]=None,
        wheel_event: Optional[MouseWheelEvent]=None,
        reopen: bool=False,
    ) -> bool:

        if self.__absolute:
            assert relative_event is None
            move_x = self.__x
            move_y = self.__y
        else:
            assert self.__x == self.__y == 0
            if relative_event is not None:
                move_x = relative_event.delta_x
                move_y = relative_event.delta_y
            else:
                move_x = move_y = 0

        if wheel_event is not None:
            wheel_x = wheel_event.delta_x
            wheel_y = wheel_event.delta_y
        else:
            wheel_x = wheel_y = 0

        report = self.__make_report(self.__pressed_buttons, move_x, move_y, wheel_x, wheel_y)
        if not self._ensure_write(report, reopen=reopen):
            self.__clear_state()
            return False
        return True

    def __clear_state(self) -> None:
        self.__pressed_buttons = 0
        self.__x = 0
        self.__y = 0

    def __make_report(self, buttons: int, move_x: int, move_y: int, wheel_x: int, wheel_y: int) -> bytes:
        # XXX: Wheel Y before X: it's ok.
        # See /kvmd/apps/otg/hid/mouse.py for details
        if self.__horizontal_wheel:
            return struct.pack(("<BHHbb" if self.__absolute else "<Bbbbb"), buttons, move_x, move_y, wheel_y, wheel_x)
        else:
            return struct.pack(("<BHHb" if self.__absolute else "<Bbbb"), buttons, move_x, move_y, wheel_y)
