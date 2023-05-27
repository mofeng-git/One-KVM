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


import dataclasses


# =====
@dataclasses.dataclass(frozen=True)
class McuKey:
    code: int


@dataclasses.dataclass(frozen=True)
class UsbKey:
    code: int
    is_modifier: bool


@dataclasses.dataclass(frozen=True)
class Key:
    mcu: McuKey
    usb: UsbKey


KEYMAP: dict[str, Key] = {
    "KeyA": Key(mcu=McuKey(code=1), usb=UsbKey(code=4, is_modifier=False)),
    "KeyB": Key(mcu=McuKey(code=2), usb=UsbKey(code=5, is_modifier=False)),
    "KeyC": Key(mcu=McuKey(code=3), usb=UsbKey(code=6, is_modifier=False)),
    "KeyD": Key(mcu=McuKey(code=4), usb=UsbKey(code=7, is_modifier=False)),
    "KeyE": Key(mcu=McuKey(code=5), usb=UsbKey(code=8, is_modifier=False)),
    "KeyF": Key(mcu=McuKey(code=6), usb=UsbKey(code=9, is_modifier=False)),
    "KeyG": Key(mcu=McuKey(code=7), usb=UsbKey(code=10, is_modifier=False)),
    "KeyH": Key(mcu=McuKey(code=8), usb=UsbKey(code=11, is_modifier=False)),
    "KeyI": Key(mcu=McuKey(code=9), usb=UsbKey(code=12, is_modifier=False)),
    "KeyJ": Key(mcu=McuKey(code=10), usb=UsbKey(code=13, is_modifier=False)),
    "KeyK": Key(mcu=McuKey(code=11), usb=UsbKey(code=14, is_modifier=False)),
    "KeyL": Key(mcu=McuKey(code=12), usb=UsbKey(code=15, is_modifier=False)),
    "KeyM": Key(mcu=McuKey(code=13), usb=UsbKey(code=16, is_modifier=False)),
    "KeyN": Key(mcu=McuKey(code=14), usb=UsbKey(code=17, is_modifier=False)),
    "KeyO": Key(mcu=McuKey(code=15), usb=UsbKey(code=18, is_modifier=False)),
    "KeyP": Key(mcu=McuKey(code=16), usb=UsbKey(code=19, is_modifier=False)),
    "KeyQ": Key(mcu=McuKey(code=17), usb=UsbKey(code=20, is_modifier=False)),
    "KeyR": Key(mcu=McuKey(code=18), usb=UsbKey(code=21, is_modifier=False)),
    "KeyS": Key(mcu=McuKey(code=19), usb=UsbKey(code=22, is_modifier=False)),
    "KeyT": Key(mcu=McuKey(code=20), usb=UsbKey(code=23, is_modifier=False)),
    "KeyU": Key(mcu=McuKey(code=21), usb=UsbKey(code=24, is_modifier=False)),
    "KeyV": Key(mcu=McuKey(code=22), usb=UsbKey(code=25, is_modifier=False)),
    "KeyW": Key(mcu=McuKey(code=23), usb=UsbKey(code=26, is_modifier=False)),
    "KeyX": Key(mcu=McuKey(code=24), usb=UsbKey(code=27, is_modifier=False)),
    "KeyY": Key(mcu=McuKey(code=25), usb=UsbKey(code=28, is_modifier=False)),
    "KeyZ": Key(mcu=McuKey(code=26), usb=UsbKey(code=29, is_modifier=False)),
    "Digit1": Key(mcu=McuKey(code=27), usb=UsbKey(code=30, is_modifier=False)),
    "Digit2": Key(mcu=McuKey(code=28), usb=UsbKey(code=31, is_modifier=False)),
    "Digit3": Key(mcu=McuKey(code=29), usb=UsbKey(code=32, is_modifier=False)),
    "Digit4": Key(mcu=McuKey(code=30), usb=UsbKey(code=33, is_modifier=False)),
    "Digit5": Key(mcu=McuKey(code=31), usb=UsbKey(code=34, is_modifier=False)),
    "Digit6": Key(mcu=McuKey(code=32), usb=UsbKey(code=35, is_modifier=False)),
    "Digit7": Key(mcu=McuKey(code=33), usb=UsbKey(code=36, is_modifier=False)),
    "Digit8": Key(mcu=McuKey(code=34), usb=UsbKey(code=37, is_modifier=False)),
    "Digit9": Key(mcu=McuKey(code=35), usb=UsbKey(code=38, is_modifier=False)),
    "Digit0": Key(mcu=McuKey(code=36), usb=UsbKey(code=39, is_modifier=False)),
    "Enter": Key(mcu=McuKey(code=37), usb=UsbKey(code=40, is_modifier=False)),
    "Escape": Key(mcu=McuKey(code=38), usb=UsbKey(code=41, is_modifier=False)),
    "Backspace": Key(mcu=McuKey(code=39), usb=UsbKey(code=42, is_modifier=False)),
    "Tab": Key(mcu=McuKey(code=40), usb=UsbKey(code=43, is_modifier=False)),
    "Space": Key(mcu=McuKey(code=41), usb=UsbKey(code=44, is_modifier=False)),
    "Minus": Key(mcu=McuKey(code=42), usb=UsbKey(code=45, is_modifier=False)),
    "Equal": Key(mcu=McuKey(code=43), usb=UsbKey(code=46, is_modifier=False)),
    "BracketLeft": Key(mcu=McuKey(code=44), usb=UsbKey(code=47, is_modifier=False)),
    "BracketRight": Key(mcu=McuKey(code=45), usb=UsbKey(code=48, is_modifier=False)),
    "Backslash": Key(mcu=McuKey(code=46), usb=UsbKey(code=49, is_modifier=False)),
    "Semicolon": Key(mcu=McuKey(code=47), usb=UsbKey(code=51, is_modifier=False)),
    "Quote": Key(mcu=McuKey(code=48), usb=UsbKey(code=52, is_modifier=False)),
    "Backquote": Key(mcu=McuKey(code=49), usb=UsbKey(code=53, is_modifier=False)),
    "Comma": Key(mcu=McuKey(code=50), usb=UsbKey(code=54, is_modifier=False)),
    "Period": Key(mcu=McuKey(code=51), usb=UsbKey(code=55, is_modifier=False)),
    "Slash": Key(mcu=McuKey(code=52), usb=UsbKey(code=56, is_modifier=False)),
    "CapsLock": Key(mcu=McuKey(code=53), usb=UsbKey(code=57, is_modifier=False)),
    "F1": Key(mcu=McuKey(code=54), usb=UsbKey(code=58, is_modifier=False)),
    "F2": Key(mcu=McuKey(code=55), usb=UsbKey(code=59, is_modifier=False)),
    "F3": Key(mcu=McuKey(code=56), usb=UsbKey(code=60, is_modifier=False)),
    "F4": Key(mcu=McuKey(code=57), usb=UsbKey(code=61, is_modifier=False)),
    "F5": Key(mcu=McuKey(code=58), usb=UsbKey(code=62, is_modifier=False)),
    "F6": Key(mcu=McuKey(code=59), usb=UsbKey(code=63, is_modifier=False)),
    "F7": Key(mcu=McuKey(code=60), usb=UsbKey(code=64, is_modifier=False)),
    "F8": Key(mcu=McuKey(code=61), usb=UsbKey(code=65, is_modifier=False)),
    "F9": Key(mcu=McuKey(code=62), usb=UsbKey(code=66, is_modifier=False)),
    "F10": Key(mcu=McuKey(code=63), usb=UsbKey(code=67, is_modifier=False)),
    "F11": Key(mcu=McuKey(code=64), usb=UsbKey(code=68, is_modifier=False)),
    "F12": Key(mcu=McuKey(code=65), usb=UsbKey(code=69, is_modifier=False)),
    "PrintScreen": Key(mcu=McuKey(code=66), usb=UsbKey(code=70, is_modifier=False)),
    "Insert": Key(mcu=McuKey(code=67), usb=UsbKey(code=73, is_modifier=False)),
    "Home": Key(mcu=McuKey(code=68), usb=UsbKey(code=74, is_modifier=False)),
    "PageUp": Key(mcu=McuKey(code=69), usb=UsbKey(code=75, is_modifier=False)),
    "Delete": Key(mcu=McuKey(code=70), usb=UsbKey(code=76, is_modifier=False)),
    "End": Key(mcu=McuKey(code=71), usb=UsbKey(code=77, is_modifier=False)),
    "PageDown": Key(mcu=McuKey(code=72), usb=UsbKey(code=78, is_modifier=False)),
    "ArrowRight": Key(mcu=McuKey(code=73), usb=UsbKey(code=79, is_modifier=False)),
    "ArrowLeft": Key(mcu=McuKey(code=74), usb=UsbKey(code=80, is_modifier=False)),
    "ArrowDown": Key(mcu=McuKey(code=75), usb=UsbKey(code=81, is_modifier=False)),
    "ArrowUp": Key(mcu=McuKey(code=76), usb=UsbKey(code=82, is_modifier=False)),
    "ControlLeft": Key(mcu=McuKey(code=77), usb=UsbKey(code=1, is_modifier=True)),
    "ShiftLeft": Key(mcu=McuKey(code=78), usb=UsbKey(code=2, is_modifier=True)),
    "AltLeft": Key(mcu=McuKey(code=79), usb=UsbKey(code=4, is_modifier=True)),
    "MetaLeft": Key(mcu=McuKey(code=80), usb=UsbKey(code=8, is_modifier=True)),
    "ControlRight": Key(mcu=McuKey(code=81), usb=UsbKey(code=16, is_modifier=True)),
    "ShiftRight": Key(mcu=McuKey(code=82), usb=UsbKey(code=32, is_modifier=True)),
    "AltRight": Key(mcu=McuKey(code=83), usb=UsbKey(code=64, is_modifier=True)),
    "MetaRight": Key(mcu=McuKey(code=84), usb=UsbKey(code=128, is_modifier=True)),
    "Pause": Key(mcu=McuKey(code=85), usb=UsbKey(code=72, is_modifier=False)),
    "ScrollLock": Key(mcu=McuKey(code=86), usb=UsbKey(code=71, is_modifier=False)),
    "NumLock": Key(mcu=McuKey(code=87), usb=UsbKey(code=83, is_modifier=False)),
    "ContextMenu": Key(mcu=McuKey(code=88), usb=UsbKey(code=101, is_modifier=False)),
    "NumpadDivide": Key(mcu=McuKey(code=89), usb=UsbKey(code=84, is_modifier=False)),
    "NumpadMultiply": Key(mcu=McuKey(code=90), usb=UsbKey(code=85, is_modifier=False)),
    "NumpadSubtract": Key(mcu=McuKey(code=91), usb=UsbKey(code=86, is_modifier=False)),
    "NumpadAdd": Key(mcu=McuKey(code=92), usb=UsbKey(code=87, is_modifier=False)),
    "NumpadEnter": Key(mcu=McuKey(code=93), usb=UsbKey(code=88, is_modifier=False)),
    "Numpad1": Key(mcu=McuKey(code=94), usb=UsbKey(code=89, is_modifier=False)),
    "Numpad2": Key(mcu=McuKey(code=95), usb=UsbKey(code=90, is_modifier=False)),
    "Numpad3": Key(mcu=McuKey(code=96), usb=UsbKey(code=91, is_modifier=False)),
    "Numpad4": Key(mcu=McuKey(code=97), usb=UsbKey(code=92, is_modifier=False)),
    "Numpad5": Key(mcu=McuKey(code=98), usb=UsbKey(code=93, is_modifier=False)),
    "Numpad6": Key(mcu=McuKey(code=99), usb=UsbKey(code=94, is_modifier=False)),
    "Numpad7": Key(mcu=McuKey(code=100), usb=UsbKey(code=95, is_modifier=False)),
    "Numpad8": Key(mcu=McuKey(code=101), usb=UsbKey(code=96, is_modifier=False)),
    "Numpad9": Key(mcu=McuKey(code=102), usb=UsbKey(code=97, is_modifier=False)),
    "Numpad0": Key(mcu=McuKey(code=103), usb=UsbKey(code=98, is_modifier=False)),
    "NumpadDecimal": Key(mcu=McuKey(code=104), usb=UsbKey(code=99, is_modifier=False)),
    "Power": Key(mcu=McuKey(code=105), usb=UsbKey(code=102, is_modifier=False)),
    "IntlBackslash": Key(mcu=McuKey(code=106), usb=UsbKey(code=100, is_modifier=False)),
    "IntlYen": Key(mcu=McuKey(code=107), usb=UsbKey(code=137, is_modifier=False)),
    "IntlRo": Key(mcu=McuKey(code=108), usb=UsbKey(code=135, is_modifier=False)),
    "KanaMode": Key(mcu=McuKey(code=109), usb=UsbKey(code=136, is_modifier=False)),
    "Convert": Key(mcu=McuKey(code=110), usb=UsbKey(code=138, is_modifier=False)),
    "NonConvert": Key(mcu=McuKey(code=111), usb=UsbKey(code=139, is_modifier=False)),
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
    65307: [At1Key(code=1, shift=False)],  # XK_Escape
    33: [At1Key(code=2, shift=True)],  # XK_exclam
    49: [At1Key(code=2, shift=False)],  # XK_1
    50: [At1Key(code=3, shift=False)],  # XK_2
    64: [At1Key(code=3, shift=True)],  # XK_at
    35: [At1Key(code=4, shift=True)],  # XK_numbersign
    51: [At1Key(code=4, shift=False)],  # XK_3
    36: [At1Key(code=5, shift=True)],  # XK_dollar
    52: [At1Key(code=5, shift=False)],  # XK_4
    37: [At1Key(code=6, shift=True)],  # XK_percent
    53: [At1Key(code=6, shift=False)],  # XK_5
    54: [At1Key(code=7, shift=False)],  # XK_6
    94: [At1Key(code=7, shift=True)],  # XK_asciicircum
    38: [At1Key(code=8, shift=True)],  # XK_ampersand
    55: [At1Key(code=8, shift=False)],  # XK_7
    42: [At1Key(code=9, shift=True)],  # XK_asterisk
    56: [At1Key(code=9, shift=False)],  # XK_8
    40: [At1Key(code=10, shift=True)],  # XK_parenleft
    57: [At1Key(code=10, shift=False)],  # XK_9
    41: [At1Key(code=11, shift=True)],  # XK_parenright
    48: [At1Key(code=11, shift=False)],  # XK_0
    45: [At1Key(code=12, shift=False)],  # XK_minus
    95: [At1Key(code=12, shift=True)],  # XK_underscore
    43: [At1Key(code=13, shift=True)],  # XK_plus
    61: [At1Key(code=13, shift=False)],  # XK_equal
    65288: [At1Key(code=14, shift=False)],  # XK_BackSpace
    65289: [At1Key(code=15, shift=False)],  # XK_Tab
    81: [At1Key(code=16, shift=True)],  # XK_Q
    113: [At1Key(code=16, shift=False)],  # XK_q
    87: [At1Key(code=17, shift=True)],  # XK_W
    119: [At1Key(code=17, shift=False)],  # XK_w
    69: [At1Key(code=18, shift=True)],  # XK_E
    101: [At1Key(code=18, shift=False)],  # XK_e
    82: [At1Key(code=19, shift=True)],  # XK_R
    114: [At1Key(code=19, shift=False)],  # XK_r
    84: [At1Key(code=20, shift=True)],  # XK_T
    116: [At1Key(code=20, shift=False)],  # XK_t
    89: [At1Key(code=21, shift=True)],  # XK_Y
    121: [At1Key(code=21, shift=False)],  # XK_y
    85: [At1Key(code=22, shift=True)],  # XK_U
    117: [At1Key(code=22, shift=False)],  # XK_u
    73: [At1Key(code=23, shift=True)],  # XK_I
    105: [At1Key(code=23, shift=False)],  # XK_i
    79: [At1Key(code=24, shift=True)],  # XK_O
    111: [At1Key(code=24, shift=False)],  # XK_o
    80: [At1Key(code=25, shift=True)],  # XK_P
    112: [At1Key(code=25, shift=False)],  # XK_p
    91: [At1Key(code=26, shift=False)],  # XK_bracketleft
    123: [At1Key(code=26, shift=True)],  # XK_braceleft
    93: [At1Key(code=27, shift=False)],  # XK_bracketright
    125: [At1Key(code=27, shift=True)],  # XK_braceright
    65293: [At1Key(code=28, shift=False)],  # XK_Return
    65507: [At1Key(code=29, shift=False)],  # XK_Control_L
    65: [At1Key(code=30, shift=True)],  # XK_A
    97: [At1Key(code=30, shift=False)],  # XK_a
    83: [At1Key(code=31, shift=True)],  # XK_S
    115: [At1Key(code=31, shift=False)],  # XK_s
    68: [At1Key(code=32, shift=True)],  # XK_D
    100: [At1Key(code=32, shift=False)],  # XK_d
    70: [At1Key(code=33, shift=True)],  # XK_F
    102: [At1Key(code=33, shift=False)],  # XK_f
    71: [At1Key(code=34, shift=True)],  # XK_G
    103: [At1Key(code=34, shift=False)],  # XK_g
    72: [At1Key(code=35, shift=True)],  # XK_H
    104: [At1Key(code=35, shift=False)],  # XK_h
    74: [At1Key(code=36, shift=True)],  # XK_J
    106: [At1Key(code=36, shift=False)],  # XK_j
    75: [At1Key(code=37, shift=True)],  # XK_K
    107: [At1Key(code=37, shift=False)],  # XK_k
    76: [At1Key(code=38, shift=True)],  # XK_L
    108: [At1Key(code=38, shift=False)],  # XK_l
    58: [At1Key(code=39, shift=True)],  # XK_colon
    59: [At1Key(code=39, shift=False)],  # XK_semicolon
    34: [At1Key(code=40, shift=True)],  # XK_quotedbl
    39: [At1Key(code=40, shift=False)],  # XK_apostrophe
    96: [At1Key(code=41, shift=False)],  # XK_grave
    126: [At1Key(code=41, shift=True)],  # XK_asciitilde
    65505: [At1Key(code=42, shift=False)],  # XK_Shift_L
    92: [At1Key(code=43, shift=False)],  # XK_backslash
    124: [At1Key(code=43, shift=True)],  # XK_bar
    90: [At1Key(code=44, shift=True)],  # XK_Z
    122: [At1Key(code=44, shift=False)],  # XK_z
    88: [At1Key(code=45, shift=True)],  # XK_X
    120: [At1Key(code=45, shift=False)],  # XK_x
    67: [At1Key(code=46, shift=True)],  # XK_C
    99: [At1Key(code=46, shift=False)],  # XK_c
    86: [At1Key(code=47, shift=True)],  # XK_V
    118: [At1Key(code=47, shift=False)],  # XK_v
    66: [At1Key(code=48, shift=True)],  # XK_B
    98: [At1Key(code=48, shift=False)],  # XK_b
    78: [At1Key(code=49, shift=True)],  # XK_N
    110: [At1Key(code=49, shift=False)],  # XK_n
    77: [At1Key(code=50, shift=True)],  # XK_M
    109: [At1Key(code=50, shift=False)],  # XK_m
    44: [At1Key(code=51, shift=False)],  # XK_comma
    60: [At1Key(code=51, shift=True)],  # XK_less
    46: [At1Key(code=52, shift=False)],  # XK_period
    62: [At1Key(code=52, shift=True)],  # XK_greater
    47: [At1Key(code=53, shift=False)],  # XK_slash
    63: [At1Key(code=53, shift=True)],  # XK_question
    65506: [At1Key(code=54, shift=False)],  # XK_Shift_R
    215: [At1Key(code=55, shift=False)],  # XK_multiply
    65513: [At1Key(code=56, shift=False)],  # XK_Alt_L
    32: [At1Key(code=57, shift=False)],  # XK_space
    65509: [At1Key(code=58, shift=False)],  # XK_Caps_Lock
    65470: [At1Key(code=59, shift=False)],  # XK_F1
    65471: [At1Key(code=60, shift=False)],  # XK_F2
    65472: [At1Key(code=61, shift=False)],  # XK_F3
    65473: [At1Key(code=62, shift=False)],  # XK_F4
    65474: [At1Key(code=63, shift=False)],  # XK_F5
    65475: [At1Key(code=64, shift=False)],  # XK_F6
    65476: [At1Key(code=65, shift=False)],  # XK_F7
    65477: [At1Key(code=66, shift=False)],  # XK_F8
    65478: [At1Key(code=67, shift=False)],  # XK_F9
    65479: [At1Key(code=68, shift=False)],  # XK_F10
    65407: [At1Key(code=69, shift=False)],  # XK_Num_Lock
    65300: [At1Key(code=70, shift=False)],  # XK_Scroll_Lock
    65463: [At1Key(code=71, shift=False)],  # XK_KP_7
    65464: [At1Key(code=72, shift=False)],  # XK_KP_8
    65465: [At1Key(code=73, shift=False)],  # XK_KP_9
    65453: [At1Key(code=74, shift=False)],  # XK_KP_Subtract
    65460: [At1Key(code=75, shift=False)],  # XK_KP_4
    65461: [At1Key(code=76, shift=False)],  # XK_KP_5
    65462: [At1Key(code=77, shift=False)],  # XK_KP_6
    65451: [At1Key(code=78, shift=False)],  # XK_KP_Add
    65457: [At1Key(code=79, shift=False)],  # XK_KP_1
    65458: [At1Key(code=80, shift=False)],  # XK_KP_2
    65459: [At1Key(code=81, shift=False)],  # XK_KP_3
    65456: [At1Key(code=82, shift=False)],  # XK_KP_0
    65454: [At1Key(code=83, shift=False)],  # XK_KP_Decimal
    65301: [At1Key(code=84, shift=False)],  # XK_Sys_Req
    65480: [At1Key(code=87, shift=False)],  # XK_F11
    65481: [At1Key(code=88, shift=False)],  # XK_F12
    65421: [At1Key(code=57372, shift=False)],  # XK_KP_Enter
    65508: [At1Key(code=57373, shift=False)],  # XK_Control_R
    65455: [At1Key(code=57397, shift=False)],  # XK_KP_Divide
    65027: [At1Key(code=57400, shift=False)],  # XK_ISO_Level3_Shift
    65514: [At1Key(code=57400, shift=False)],  # XK_Alt_R
    65299: [At1Key(code=57414, shift=False)],  # XK_Pause
    65360: [At1Key(code=57415, shift=False)],  # XK_Home
    65362: [At1Key(code=57416, shift=False)],  # XK_Up
    65365: [At1Key(code=57417, shift=False)],  # XK_Page_Up
    65361: [At1Key(code=57419, shift=False)],  # XK_Left
    65363: [At1Key(code=57421, shift=False)],  # XK_Right
    65367: [At1Key(code=57423, shift=False)],  # XK_End
    65364: [At1Key(code=57424, shift=False)],  # XK_Down
    65366: [At1Key(code=57425, shift=False)],  # XK_Page_Down
    65379: [At1Key(code=57426, shift=False)],  # XK_Insert
    65535: [At1Key(code=57427, shift=False)],  # XK_Delete
    65511: [At1Key(code=57435, shift=False)],  # XK_Meta_L
    65515: [At1Key(code=57435, shift=False)],  # XK_Super_L
    65512: [At1Key(code=57436, shift=False)],  # XK_Meta_R
    65516: [At1Key(code=57436, shift=False)],  # XK_Super_R
    65383: [At1Key(code=57437, shift=False)],  # XK_Menu
    269025071: [At1Key(code=57438, shift=False)],  # XK_XF86_Sleep
}


AT1_TO_WEB = {
    1: "Escape",
    2: "Digit1",
    3: "Digit2",
    4: "Digit3",
    5: "Digit4",
    6: "Digit5",
    7: "Digit6",
    8: "Digit7",
    9: "Digit8",
    10: "Digit9",
    11: "Digit0",
    12: "Minus",
    13: "Equal",
    14: "Backspace",
    15: "Tab",
    16: "KeyQ",
    17: "KeyW",
    18: "KeyE",
    19: "KeyR",
    20: "KeyT",
    21: "KeyY",
    22: "KeyU",
    23: "KeyI",
    24: "KeyO",
    25: "KeyP",
    26: "BracketLeft",
    27: "BracketRight",
    28: "Enter",
    29: "ControlLeft",
    30: "KeyA",
    31: "KeyS",
    32: "KeyD",
    33: "KeyF",
    34: "KeyG",
    35: "KeyH",
    36: "KeyJ",
    37: "KeyK",
    38: "KeyL",
    39: "Semicolon",
    40: "Quote",
    41: "Backquote",
    42: "ShiftLeft",
    43: "Backslash",
    44: "KeyZ",
    45: "KeyX",
    46: "KeyC",
    47: "KeyV",
    48: "KeyB",
    49: "KeyN",
    50: "KeyM",
    51: "Comma",
    52: "Period",
    53: "Slash",
    54: "ShiftRight",
    55: "NumpadMultiply",
    56: "AltLeft",
    57: "Space",
    58: "CapsLock",
    59: "F1",
    60: "F2",
    61: "F3",
    62: "F4",
    63: "F5",
    64: "F6",
    65: "F7",
    66: "F8",
    67: "F9",
    68: "F10",
    69: "NumLock",
    70: "ScrollLock",
    71: "Numpad7",
    72: "Numpad8",
    73: "Numpad9",
    74: "NumpadSubtract",
    75: "Numpad4",
    76: "Numpad5",
    77: "Numpad6",
    78: "NumpadAdd",
    79: "Numpad1",
    80: "Numpad2",
    81: "Numpad3",
    82: "Numpad0",
    83: "NumpadDecimal",
    84: "PrintScreen",
    86: "IntlBackslash",
    87: "F11",
    88: "F12",
    112: "KanaMode",
    115: "IntlRo",
    121: "Convert",
    123: "NonConvert",
    125: "IntlYen",
    57372: "NumpadEnter",
    57373: "ControlRight",
    57397: "NumpadDivide",
    57400: "AltRight",
    57414: "Pause",
    57415: "Home",
    57416: "ArrowUp",
    57417: "PageUp",
    57419: "ArrowLeft",
    57421: "ArrowRight",
    57423: "End",
    57424: "ArrowDown",
    57425: "PageDown",
    57426: "Insert",
    57427: "Delete",
    57435: "MetaLeft",
    57436: "MetaRight",
    57437: "ContextMenu",
    57438: "Power",
}
