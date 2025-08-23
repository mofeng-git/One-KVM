# ========================================================================== #
#                                                                            #
#    KVMD - The main PiKVM daemon.                                           #
#                                                                            #
#    Copyright (C) 2018-2024  Maxim Devaev <mdevaev@gmail.com>               #
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


import asyncio

from typing import Final
from typing import Generator

import evdev
from evdev import ecodes


# =====
class Hid:  # pylint: disable=too-many-instance-attributes
    KEY:          Final[int] = 0
    MOUSE_BUTTON: Final[int] = 1
    MOUSE_REL:    Final[int] = 2
    MOUSE_WHEEL:  Final[int] = 3

    def __init__(self, path: str) -> None:
        self.__device = evdev.InputDevice(path)

        caps = self.__device.capabilities(absinfo=False)

        syns = caps.get(ecodes.EV_SYN, [])
        self.__has_syn = (ecodes.SYN_REPORT in syns)

        leds = caps.get(ecodes.EV_LED, [])
        self.__has_caps = (ecodes.LED_CAPSL in leds)
        self.__has_scroll = (ecodes.LED_SCROLLL in leds)
        self.__has_num = (ecodes.LED_NUML in leds)

        keys = caps.get(ecodes.EV_KEY, [])
        self.__has_keyboard = (
            ecodes.KEY_LEFTCTRL in keys
            or ecodes.KEY_RIGHTCTRL in keys
            or ecodes.KEY_LEFTSHIFT in keys
            or ecodes.KEY_RIGHTSHIFT in keys
        )

        rels = caps.get(ecodes.EV_REL, [])
        self.__has_mouse_rel = (
            ecodes.BTN_LEFT in keys
            and ecodes.REL_X in rels
        )

        self.__grabbed = False

    def is_suitable(self) -> bool:
        return (self.__has_keyboard or self.__has_mouse_rel)

    def set_leds(self, caps: bool, scroll: bool, num: bool) -> None:
        if self.__grabbed:
            if self.__has_caps:
                self.__device.set_led(ecodes.LED_CAPSL, caps)
            if self.__has_scroll:
                self.__device.set_led(ecodes.LED_SCROLLL, scroll)
            if self.__has_num:
                self.__device.set_led(ecodes.LED_NUML, num)

    def set_grabbed(self, grabbed: bool) -> None:
        if self.__grabbed != grabbed:
            getattr(self.__device, ("grab" if grabbed else "ungrab"))()
            self.__grabbed = grabbed

    def close(self) -> None:
        try:
            self.__device.close()
        except Exception:
            pass

    async def poll_to_queue(self, queue: asyncio.Queue[tuple[int, tuple]]) -> None:
        def put(event: int, args: tuple) -> None:
            queue.put_nowait((event, args))

        move_x = move_y = 0
        wheel_x = wheel_y = 0
        async for event in self.__device.async_read_loop():
            if not self.__grabbed:
                # Клавиши перехватываются всегда для обработки хоткеев,
                # всё остальное пропускается для экономии ресурсов.
                if event.type == ecodes.EV_KEY and event.value != 2 and (event.code in ecodes.KEY):
                    put(self.KEY, (event.code, bool(event.value)))
                continue

            if event.type == ecodes.EV_REL:
                match event.code:
                    case ecodes.REL_X:
                        move_x += event.value
                    case ecodes.REL_Y:
                        move_y += event.value
                    case ecodes.REL_HWHEEL:
                        wheel_x += event.value
                    case ecodes.REL_WHEEL:
                        wheel_y += event.value

            if not self.__has_syn or event.type == ecodes.SYN_REPORT:
                if move_x or move_y:
                    for xy in self.__splitted_deltas(move_x, move_y):
                        put(self.MOUSE_REL, xy)
                    move_x = move_y = 0
                if wheel_x or wheel_y:
                    for xy in self.__splitted_deltas(wheel_x, wheel_y):
                        put(self.MOUSE_WHEEL, xy)
                    wheel_x = wheel_y = 0

            elif event.type == ecodes.EV_KEY and event.value != 2:
                if event.code in ecodes.KEY:
                    put(self.KEY, (event.code, bool(event.value)))
                elif event.code in ecodes.BTN:
                    put(self.MOUSE_BUTTON, (event.code, bool(event.value)))

    def __splitted_deltas(self, delta_x: int, delta_y: int) -> Generator[tuple[int, int], None, None]:
        sign_x = (-1 if delta_x < 0 else 1)
        sign_y = (-1 if delta_y < 0 else 1)
        delta_x = abs(delta_x)
        delta_y = abs(delta_y)
        while delta_x > 0 or delta_y > 0:
            dx = sign_x * max(min(delta_x, 127), 0)
            dy = sign_y * max(min(delta_y, 127), 0)
            yield (dx, dy)
            delta_x -= 127
            delta_y -= 127

    def __str__(self) -> str:
        info: list[str] = []
        if self.__has_syn:
            info.append("syn")
        if self.__has_keyboard:
            info.append("keyboard")
        if self.__has_mouse_rel:
            info.append("mouse_rel")
        return f"Hid({self.__device.path!r}, {self.__device.name!r}, {self.__device.phys!r}, {', '.join(info)})"
