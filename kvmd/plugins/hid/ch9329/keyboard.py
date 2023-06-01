# ========================================================================== #
#                                                                            #
#    KVMD - The main PiKVM daemon.                                           #
#                                                                            #
#    Copyright (C) 2018-2022  Maxim Devaev <mdevaev@gmail.com>               #
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


from .... import aiomulti

from ....keyboard.mappings import KEYMAP


# =====
class Keyboard:
    def __init__(self) -> None:
        self.__leds = aiomulti.AioSharedFlags({
            "num": False,
            "caps": False,
            "scroll": False,
        }, aiomulti.AioProcessNotifier(), bool)
        self.__modifiers = 0
        self.__active_keys: list[int] = []

    def set_leds(self, led_byte: int) -> None:
        self.__leds.update(
            num=bool(led_byte & 1),
            caps=bool((led_byte >> 1) & 1),
            scroll=bool((led_byte >> 2) & 1),
        )

    async def get_leds(self) -> dict[str, bool]:
        return (await self.__leds.get())

    def process_key(self, key: str, state: bool) -> bytes:
        code = KEYMAP[key].usb.code
        is_modifier = KEYMAP[key].usb.is_modifier
        if state:
            if is_modifier:
                self.__modifiers |= code
            elif len(self.__active_keys) < 6 and code not in self.__active_keys:
                self.__active_keys.append(code)
        else:
            if is_modifier:
                self.__modifiers &= ~code
            elif code in self.__active_keys:
                self.__active_keys.remove(code)
        cmd = [
            0, 0x02, 0x08, self.__modifiers, 0,
            0, 0, 0, 0, 0, 0,
        ]
        for (index, code) in enumerate(self.__active_keys):
            cmd[index + 5] = code
        return bytes(cmd)
