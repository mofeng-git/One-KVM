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


from typing import Generator
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
from .events import make_mouse_report


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

        self.__pressed_buttons = 0
        self.__x = 0  # For absolute
        self.__y = 0
        self.__win98_fix = False

    def is_absolute(self) -> bool:
        return self.__absolute

    def set_win98_fix(self, enabled: bool) -> None:
        self.__win98_fix = enabled

    def get_win98_fix(self) -> bool:
        return self.__win98_fix

    def cleanup(self) -> None:
        self._stop()
        get_logger().info("Clearing HID-mouse events ...")
        report = make_mouse_report(
            absolute=self.__absolute,
            buttons=0,
            move_x=(self.__x if self.__absolute else 0),
            move_y=(self.__y if self.__absolute else 0),
            wheel_x=(0 if self.__horizontal_wheel else None),
            wheel_y=0,
        )
        self._cleanup_write(report)  # Release all buttons

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
            self._queue_event(MouseMoveEvent(to_x, to_y, self.__win98_fix))

    def send_relative_event(self, delta_x: int, delta_y: int) -> None:
        if not self.__absolute:
            self._queue_event(MouseRelativeEvent(delta_x, delta_y))

    def send_wheel_event(self, delta_x: int, delta_y: int) -> None:
        self._queue_event(MouseWheelEvent(delta_x, delta_y))

    # =====

    def _process_event(self, event: BaseEvent) -> Generator[bytes, None, None]:
        if isinstance(event, (ClearEvent, ResetEvent)):
            yield self.__process_clear_event()
        elif isinstance(event, MouseButtonEvent):
            yield from self.__process_button_event(event)
        elif isinstance(event, MouseMoveEvent):
            yield self.__process_move_event(event)
        elif isinstance(event, MouseRelativeEvent):
            yield self.__process_relative_event(event)
        elif isinstance(event, MouseWheelEvent):
            yield self.__process_wheel_event(event)
        else:
            raise RuntimeError(f"Not implemented event: {event}")

    def __process_clear_event(self) -> bytes:
        self.__clear_state()
        return self.__make_report()

    def __process_button_event(self, event: MouseButtonEvent) -> Generator[bytes, None, None]:
        if event.code & self.__pressed_buttons:
            # Ранее нажатую кнопку отжимаем
            self.__pressed_buttons &= ~event.code
            yield self.__make_report()
        if event.state:
            # Нажимаем если нужно
            self.__pressed_buttons |= event.code
            yield self.__make_report()

    def __process_move_event(self, event: MouseMoveEvent) -> bytes:
        self.__x = event.to_fixed_x
        self.__y = event.to_fixed_y
        return self.__make_report()

    def __process_relative_event(self, event: MouseRelativeEvent) -> bytes:
        return self.__make_report(relative_event=event)

    def __process_wheel_event(self, event: MouseWheelEvent) -> bytes:
        return self.__make_report(wheel_event=event)

    # =====

    def __make_report(
        self,
        relative_event: (MouseRelativeEvent | None)=None,
        wheel_event: (MouseWheelEvent | None)=None,
    ) -> bytes:

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

        return make_mouse_report(
            absolute=self.__absolute,
            buttons=self.__pressed_buttons,
            move_x=move_x,
            move_y=move_y,
            wheel_x=(wheel_x if self.__horizontal_wheel else None),
            wheel_y=wheel_y,
        )

    def __clear_state(self) -> None:
        self.__pressed_buttons = 0
        self.__x = 0
        self.__y = 0
