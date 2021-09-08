# ========================================================================== #
#                                                                            #
#    KVMD - The main PiKVM daemon.                                           #
#                                                                            #
#    Copyright (C) 2018-2021  Maxim Devaev <mdevaev@gmail.com>               #
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

from typing import Dict


# =====
@dataclasses.dataclass(frozen=True)
class McuKey:
    code: int


@dataclasses.dataclass(frozen=True)
class OtgKey:
    code: int
    is_modifier: bool


@dataclasses.dataclass(frozen=True)
class Key:
    mcu: McuKey
    otg: OtgKey

<%! import operator %>
KEYMAP: Dict[str, Key] = {
% for km in sorted(keymap, key=operator.attrgetter("mcu_code")):
    "${km.web_name}": Key(mcu=McuKey(code=${km.mcu_code}), otg=OtgKey(code=${km.otg_key.code}, is_modifier=${km.otg_key.is_modifier})),
% endfor
}


# =====
class WebModifiers:
    SHIFT_LEFT = "ShiftLeft"
    SHIFT_RIGHT = "ShiftRight"
    SHIFTS = set([SHIFT_LEFT, SHIFT_RIGHT])

    ALT_LEFT = "AltLeft"
    ALT_RIGHT = "AltRight"
    ALTS = set([ALT_LEFT, ALT_RIGHT])

    CTRL_LEFT = "ControlLeft"
    CTRL_RIGHT = "ControlRight"
    CTRLS = set([CTRL_RIGHT, CTRL_RIGHT])


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
    code: int
    shift: bool
    altgr: bool = False
    ctrl: bool = False


X11_TO_AT1 = {
% for km in sorted(keymap, key=operator.attrgetter("at1_code")):
    % for x11_key in sorted(km.x11_keys, key=(lambda key: (key.code, key.shift))):
    ${x11_key.code}: [At1Key(code=${km.at1_code}, shift=${x11_key.shift})],  # ${x11_key.name}
    % endfor
% endfor
}


AT1_TO_WEB = {
% for km in sorted(keymap, key=operator.attrgetter("at1_code")):
    ${km.at1_code}: "${km.web_name}",
% endfor
}
