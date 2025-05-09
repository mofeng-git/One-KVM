# ========================================================================== #
#                                                                            #
#    KVMD - The main PiKVM daemon.                                           #
#                                                                            #
#    Copyright (C) 2020  Maxim Devaev <mdevaev@gmail.com>                    #
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


import time

from evdev import ecodes


# =====
class BaseMagicHandler:
    __MAGIC_KEY = ecodes.KEY_LEFTALT
    __MAGIC_TIMEOUT = 2
    __MAGIC_TRIGGER = 2

    def __init__(self) -> None:
        self.__taps = 0
        self.__ts = 0.0
        self.__codes: list[int] = []

    async def _magic_handle_key(self, key: int, state: bool) -> None:  # pylint: disable=too-many-branches
        if self.__ts + self.__MAGIC_TIMEOUT < time.monotonic():
            self.__taps = 0
            self.__ts = 0
            self.__codes = []

        if key == self.__MAGIC_KEY:
            if not state:
                self.__taps += 1
                self.__ts = time.monotonic()
        elif state:
            taps = self.__taps
            codes = self.__codes
            self.__taps = 0
            self.__ts = 0
            self.__codes = []
            if taps >= self.__MAGIC_TRIGGER:
                if key == ecodes.KEY_P:
                    await self._on_magic_clipboard_print()
                    return

                elif key in [ecodes.KEY_UP, ecodes.KEY_LEFT]:
                    await self._on_magic_switch_prev()
                    return

                elif key in [ecodes.KEY_DOWN, ecodes.KEY_RIGHT]:
                    await self._on_magic_switch_next()
                    return

                elif ecodes.KEY_1 <= key <= ecodes.KEY_8:
                    codes.append(key - ecodes.KEY_1)
                    if len(codes) == 1:
                        if not (await self._on_magic_switch_port(codes[0], -1)):
                            self.__taps = taps
                            self.__ts = time.monotonic()
                            self.__codes = codes
                    elif len(codes) >= 2:
                        await self._on_magic_switch_port(codes[0], codes[1])
                    return

        await self._on_magic_key_proxy(key, state)

    async def _on_magic_clipboard_print(self) -> None:
        pass

    async def _on_magic_switch_prev(self) -> None:
        pass

    async def _on_magic_switch_next(self) -> None:
        pass

    async def _on_magic_switch_port(self, first: int, second: int) -> bool:
        _ = first
        _ = second
        return True

    async def _on_magic_key_proxy(self, key: int, state: bool) -> None:
        raise NotImplementedError()
