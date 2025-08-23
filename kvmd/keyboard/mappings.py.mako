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


import dataclasses

from evdev import ecodes


# =====
@dataclasses.dataclass(frozen=True)
class McuKey:
    code: int


@dataclasses.dataclass(frozen=True)
class UsbKey:
    code:        int
    is_modifier: bool


@dataclasses.dataclass(frozen=True)
class Key:
    mcu: McuKey
    usb: UsbKey

<%! import operator %>
KEYMAP: dict[int, Key] = {
% for km in sorted(keymap, key=operator.attrgetter("mcu_code")):
    ecodes.${km.evdev_name}: Key(mcu=McuKey(code=${km.mcu_code}), usb=UsbKey(code=${km.usb_key.code}, is_modifier=${km.usb_key.is_modifier})),
% endfor
}


WEB_TO_EVDEV = {
% for km in sorted(keymap, key=operator.attrgetter("mcu_code")):
    "${km.web_name}": ecodes.${km.evdev_name},
% endfor
}


# =====
class EvdevModifiers:
    SHIFT_LEFT = ecodes.KEY_LEFTSHIFT
    SHIFT_RIGHT = ecodes.KEY_RIGHTSHIFT
    SHIFTS = set([SHIFT_LEFT, SHIFT_RIGHT])

    ALT_LEFT = ecodes.KEY_LEFTALT
    ALT_RIGHT = ecodes.KEY_RIGHTALT
    ALTS = set([ALT_LEFT, ALT_RIGHT])

    CTRL_LEFT = ecodes.KEY_LEFTCTRL
    CTRL_RIGHT = ecodes.KEY_RIGHTCTRL
    CTRLS = set([CTRL_LEFT, CTRL_RIGHT])

    META_LEFT = ecodes.KEY_LEFTMETA
    META_RIGHT = ecodes.KEY_RIGHTMETA
    METAS = set([META_LEFT, META_RIGHT])

    ALL = (SHIFTS | ALTS | CTRLS | METAS)


class X11Modifiers:
    SHIFT_LEFT = 65505
    SHIFT_RIGHT = 65506
    SHIFTS = set([SHIFT_LEFT, SHIFT_RIGHT])

    ALTGR = 65027  # XK_ISO_Level3_Shift

    CTRL_LEFT = 65507
    CTRL_RIGHT = 65508
    CTRLS = set([CTRL_LEFT, CTRL_RIGHT])


# =====
@dataclasses.dataclass(frozen=True)
class At1Key:
    code:  int
    shift: bool
    altgr: bool = False
    ctrl:  bool = False


X11_TO_AT1 = {
% for km in sorted(keymap, key=operator.attrgetter("at1_code")):
    % for x11_key in sorted(km.x11_keys, key=(lambda key: (key.code, key.shift))):
    ${x11_key.code}: [At1Key(code=${km.at1_code}, shift=${x11_key.shift})],  # ${x11_key.name}
    % endfor
% endfor
}


AT1_TO_EVDEV = {
% for km in sorted(keymap, key=operator.attrgetter("at1_code")):
    ${km.at1_code}: ecodes.${km.evdev_name},
% endfor
}
