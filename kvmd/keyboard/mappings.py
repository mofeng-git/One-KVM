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

from typing import Dict


# =====
@dataclasses.dataclass(frozen=True)
class SerialKey:
    code: int


@dataclasses.dataclass(frozen=True)
class OtgKey:
    code: int
    is_modifier: bool


@dataclasses.dataclass(frozen=True)
class Key:
    serial: SerialKey
    otg: OtgKey


KEYMAP: Dict[str, Key] = {
    "KeyA": Key(serial=SerialKey(code=1), otg=OtgKey(code=4, is_modifier=False)),
    "KeyB": Key(serial=SerialKey(code=2), otg=OtgKey(code=5, is_modifier=False)),
    "KeyC": Key(serial=SerialKey(code=3), otg=OtgKey(code=6, is_modifier=False)),
    "KeyD": Key(serial=SerialKey(code=4), otg=OtgKey(code=7, is_modifier=False)),
    "KeyE": Key(serial=SerialKey(code=5), otg=OtgKey(code=8, is_modifier=False)),
    "KeyF": Key(serial=SerialKey(code=6), otg=OtgKey(code=9, is_modifier=False)),
    "KeyG": Key(serial=SerialKey(code=7), otg=OtgKey(code=10, is_modifier=False)),
    "KeyH": Key(serial=SerialKey(code=8), otg=OtgKey(code=11, is_modifier=False)),
    "KeyI": Key(serial=SerialKey(code=9), otg=OtgKey(code=12, is_modifier=False)),
    "KeyJ": Key(serial=SerialKey(code=10), otg=OtgKey(code=13, is_modifier=False)),
    "KeyK": Key(serial=SerialKey(code=11), otg=OtgKey(code=14, is_modifier=False)),
    "KeyL": Key(serial=SerialKey(code=12), otg=OtgKey(code=15, is_modifier=False)),
    "KeyM": Key(serial=SerialKey(code=13), otg=OtgKey(code=16, is_modifier=False)),
    "KeyN": Key(serial=SerialKey(code=14), otg=OtgKey(code=17, is_modifier=False)),
    "KeyO": Key(serial=SerialKey(code=15), otg=OtgKey(code=18, is_modifier=False)),
    "KeyP": Key(serial=SerialKey(code=16), otg=OtgKey(code=19, is_modifier=False)),
    "KeyQ": Key(serial=SerialKey(code=17), otg=OtgKey(code=20, is_modifier=False)),
    "KeyR": Key(serial=SerialKey(code=18), otg=OtgKey(code=21, is_modifier=False)),
    "KeyS": Key(serial=SerialKey(code=19), otg=OtgKey(code=22, is_modifier=False)),
    "KeyT": Key(serial=SerialKey(code=20), otg=OtgKey(code=23, is_modifier=False)),
    "KeyU": Key(serial=SerialKey(code=21), otg=OtgKey(code=24, is_modifier=False)),
    "KeyV": Key(serial=SerialKey(code=22), otg=OtgKey(code=25, is_modifier=False)),
    "KeyW": Key(serial=SerialKey(code=23), otg=OtgKey(code=26, is_modifier=False)),
    "KeyX": Key(serial=SerialKey(code=24), otg=OtgKey(code=27, is_modifier=False)),
    "KeyY": Key(serial=SerialKey(code=25), otg=OtgKey(code=28, is_modifier=False)),
    "KeyZ": Key(serial=SerialKey(code=26), otg=OtgKey(code=29, is_modifier=False)),
    "Digit1": Key(serial=SerialKey(code=27), otg=OtgKey(code=30, is_modifier=False)),
    "Digit2": Key(serial=SerialKey(code=28), otg=OtgKey(code=31, is_modifier=False)),
    "Digit3": Key(serial=SerialKey(code=29), otg=OtgKey(code=32, is_modifier=False)),
    "Digit4": Key(serial=SerialKey(code=30), otg=OtgKey(code=33, is_modifier=False)),
    "Digit5": Key(serial=SerialKey(code=31), otg=OtgKey(code=34, is_modifier=False)),
    "Digit6": Key(serial=SerialKey(code=32), otg=OtgKey(code=35, is_modifier=False)),
    "Digit7": Key(serial=SerialKey(code=33), otg=OtgKey(code=36, is_modifier=False)),
    "Digit8": Key(serial=SerialKey(code=34), otg=OtgKey(code=37, is_modifier=False)),
    "Digit9": Key(serial=SerialKey(code=35), otg=OtgKey(code=38, is_modifier=False)),
    "Digit0": Key(serial=SerialKey(code=36), otg=OtgKey(code=39, is_modifier=False)),
    "Enter": Key(serial=SerialKey(code=37), otg=OtgKey(code=40, is_modifier=False)),
    "Escape": Key(serial=SerialKey(code=38), otg=OtgKey(code=41, is_modifier=False)),
    "Backspace": Key(serial=SerialKey(code=39), otg=OtgKey(code=42, is_modifier=False)),
    "Tab": Key(serial=SerialKey(code=40), otg=OtgKey(code=43, is_modifier=False)),
    "Space": Key(serial=SerialKey(code=41), otg=OtgKey(code=44, is_modifier=False)),
    "Minus": Key(serial=SerialKey(code=42), otg=OtgKey(code=45, is_modifier=False)),
    "Equal": Key(serial=SerialKey(code=43), otg=OtgKey(code=46, is_modifier=False)),
    "BracketLeft": Key(serial=SerialKey(code=44), otg=OtgKey(code=47, is_modifier=False)),
    "BracketRight": Key(serial=SerialKey(code=45), otg=OtgKey(code=48, is_modifier=False)),
    "Backslash": Key(serial=SerialKey(code=46), otg=OtgKey(code=49, is_modifier=False)),
    "Semicolon": Key(serial=SerialKey(code=47), otg=OtgKey(code=51, is_modifier=False)),
    "Quote": Key(serial=SerialKey(code=48), otg=OtgKey(code=52, is_modifier=False)),
    "Backquote": Key(serial=SerialKey(code=49), otg=OtgKey(code=53, is_modifier=False)),
    "Comma": Key(serial=SerialKey(code=50), otg=OtgKey(code=54, is_modifier=False)),
    "Period": Key(serial=SerialKey(code=51), otg=OtgKey(code=55, is_modifier=False)),
    "Slash": Key(serial=SerialKey(code=52), otg=OtgKey(code=56, is_modifier=False)),
    "CapsLock": Key(serial=SerialKey(code=53), otg=OtgKey(code=57, is_modifier=False)),
    "F1": Key(serial=SerialKey(code=54), otg=OtgKey(code=58, is_modifier=False)),
    "F2": Key(serial=SerialKey(code=55), otg=OtgKey(code=59, is_modifier=False)),
    "F3": Key(serial=SerialKey(code=56), otg=OtgKey(code=60, is_modifier=False)),
    "F4": Key(serial=SerialKey(code=57), otg=OtgKey(code=61, is_modifier=False)),
    "F5": Key(serial=SerialKey(code=58), otg=OtgKey(code=62, is_modifier=False)),
    "F6": Key(serial=SerialKey(code=59), otg=OtgKey(code=63, is_modifier=False)),
    "F7": Key(serial=SerialKey(code=60), otg=OtgKey(code=64, is_modifier=False)),
    "F8": Key(serial=SerialKey(code=61), otg=OtgKey(code=65, is_modifier=False)),
    "F9": Key(serial=SerialKey(code=62), otg=OtgKey(code=66, is_modifier=False)),
    "F10": Key(serial=SerialKey(code=63), otg=OtgKey(code=67, is_modifier=False)),
    "F11": Key(serial=SerialKey(code=64), otg=OtgKey(code=68, is_modifier=False)),
    "F12": Key(serial=SerialKey(code=65), otg=OtgKey(code=69, is_modifier=False)),
    "PrintScreen": Key(serial=SerialKey(code=66), otg=OtgKey(code=70, is_modifier=False)),
    "Insert": Key(serial=SerialKey(code=67), otg=OtgKey(code=73, is_modifier=False)),
    "Home": Key(serial=SerialKey(code=68), otg=OtgKey(code=74, is_modifier=False)),
    "PageUp": Key(serial=SerialKey(code=69), otg=OtgKey(code=75, is_modifier=False)),
    "Delete": Key(serial=SerialKey(code=70), otg=OtgKey(code=76, is_modifier=False)),
    "End": Key(serial=SerialKey(code=71), otg=OtgKey(code=77, is_modifier=False)),
    "PageDown": Key(serial=SerialKey(code=72), otg=OtgKey(code=78, is_modifier=False)),
    "ArrowRight": Key(serial=SerialKey(code=73), otg=OtgKey(code=79, is_modifier=False)),
    "ArrowLeft": Key(serial=SerialKey(code=74), otg=OtgKey(code=80, is_modifier=False)),
    "ArrowDown": Key(serial=SerialKey(code=75), otg=OtgKey(code=81, is_modifier=False)),
    "ArrowUp": Key(serial=SerialKey(code=76), otg=OtgKey(code=82, is_modifier=False)),
    "ControlLeft": Key(serial=SerialKey(code=77), otg=OtgKey(code=1, is_modifier=True)),
    "ShiftLeft": Key(serial=SerialKey(code=78), otg=OtgKey(code=2, is_modifier=True)),
    "AltLeft": Key(serial=SerialKey(code=79), otg=OtgKey(code=4, is_modifier=True)),
    "MetaLeft": Key(serial=SerialKey(code=80), otg=OtgKey(code=8, is_modifier=True)),
    "ControlRight": Key(serial=SerialKey(code=81), otg=OtgKey(code=16, is_modifier=True)),
    "ShiftRight": Key(serial=SerialKey(code=82), otg=OtgKey(code=32, is_modifier=True)),
    "AltRight": Key(serial=SerialKey(code=83), otg=OtgKey(code=64, is_modifier=True)),
    "MetaRight": Key(serial=SerialKey(code=84), otg=OtgKey(code=128, is_modifier=True)),
    "Pause": Key(serial=SerialKey(code=85), otg=OtgKey(code=72, is_modifier=False)),
    "ScrollLock": Key(serial=SerialKey(code=86), otg=OtgKey(code=71, is_modifier=False)),
    "NumLock": Key(serial=SerialKey(code=87), otg=OtgKey(code=83, is_modifier=False)),
    "ContextMenu": Key(serial=SerialKey(code=88), otg=OtgKey(code=101, is_modifier=False)),
}


# =====
@dataclasses.dataclass(frozen=True)
class At1Key:
    code: int
    shift: bool
    altgr: bool = False
    ctrl: bool = False


X11_TO_AT1 = {
    65307: At1Key(code=1, shift=False),  # XK_Escape
    33: At1Key(code=2, shift=True),  # XK_exclam
    49: At1Key(code=2, shift=False),  # XK_1
    50: At1Key(code=3, shift=False),  # XK_2
    64: At1Key(code=3, shift=True),  # XK_at
    35: At1Key(code=4, shift=True),  # XK_numbersign
    51: At1Key(code=4, shift=False),  # XK_3
    36: At1Key(code=5, shift=True),  # XK_dollar
    52: At1Key(code=5, shift=False),  # XK_4
    37: At1Key(code=6, shift=True),  # XK_percent
    53: At1Key(code=6, shift=False),  # XK_5
    54: At1Key(code=7, shift=False),  # XK_6
    94: At1Key(code=7, shift=True),  # XK_asciicircum
    38: At1Key(code=8, shift=True),  # XK_ampersand
    55: At1Key(code=8, shift=False),  # XK_7
    42: At1Key(code=9, shift=True),  # XK_asterisk
    56: At1Key(code=9, shift=False),  # XK_8
    40: At1Key(code=10, shift=True),  # XK_parenleft
    57: At1Key(code=10, shift=False),  # XK_9
    41: At1Key(code=11, shift=True),  # XK_parenright
    48: At1Key(code=11, shift=False),  # XK_0
    45: At1Key(code=12, shift=False),  # XK_minus
    95: At1Key(code=12, shift=True),  # XK_underscore
    43: At1Key(code=13, shift=True),  # XK_plus
    61: At1Key(code=13, shift=False),  # XK_equal
    65288: At1Key(code=14, shift=False),  # XK_BackSpace
    65289: At1Key(code=15, shift=False),  # XK_Tab
    81: At1Key(code=16, shift=True),  # XK_Q
    113: At1Key(code=16, shift=False),  # XK_q
    87: At1Key(code=17, shift=True),  # XK_W
    119: At1Key(code=17, shift=False),  # XK_w
    69: At1Key(code=18, shift=True),  # XK_E
    101: At1Key(code=18, shift=False),  # XK_e
    82: At1Key(code=19, shift=True),  # XK_R
    114: At1Key(code=19, shift=False),  # XK_r
    84: At1Key(code=20, shift=True),  # XK_T
    116: At1Key(code=20, shift=False),  # XK_t
    89: At1Key(code=21, shift=True),  # XK_Y
    121: At1Key(code=21, shift=False),  # XK_y
    85: At1Key(code=22, shift=True),  # XK_U
    117: At1Key(code=22, shift=False),  # XK_u
    73: At1Key(code=23, shift=True),  # XK_I
    105: At1Key(code=23, shift=False),  # XK_i
    79: At1Key(code=24, shift=True),  # XK_O
    111: At1Key(code=24, shift=False),  # XK_o
    80: At1Key(code=25, shift=True),  # XK_P
    112: At1Key(code=25, shift=False),  # XK_p
    91: At1Key(code=26, shift=False),  # XK_bracketleft
    123: At1Key(code=26, shift=True),  # XK_braceleft
    93: At1Key(code=27, shift=False),  # XK_bracketright
    125: At1Key(code=27, shift=True),  # XK_braceright
    65293: At1Key(code=28, shift=False),  # XK_Return
    65507: At1Key(code=29, shift=False),  # XK_Control_L
    65: At1Key(code=30, shift=True),  # XK_A
    97: At1Key(code=30, shift=False),  # XK_a
    83: At1Key(code=31, shift=True),  # XK_S
    115: At1Key(code=31, shift=False),  # XK_s
    68: At1Key(code=32, shift=True),  # XK_D
    100: At1Key(code=32, shift=False),  # XK_d
    70: At1Key(code=33, shift=True),  # XK_F
    102: At1Key(code=33, shift=False),  # XK_f
    71: At1Key(code=34, shift=True),  # XK_G
    103: At1Key(code=34, shift=False),  # XK_g
    72: At1Key(code=35, shift=True),  # XK_H
    104: At1Key(code=35, shift=False),  # XK_h
    74: At1Key(code=36, shift=True),  # XK_J
    106: At1Key(code=36, shift=False),  # XK_j
    75: At1Key(code=37, shift=True),  # XK_K
    107: At1Key(code=37, shift=False),  # XK_k
    76: At1Key(code=38, shift=True),  # XK_L
    108: At1Key(code=38, shift=False),  # XK_l
    58: At1Key(code=39, shift=True),  # XK_colon
    59: At1Key(code=39, shift=False),  # XK_semicolon
    34: At1Key(code=40, shift=True),  # XK_quotedbl
    39: At1Key(code=40, shift=False),  # XK_apostrophe
    96: At1Key(code=41, shift=False),  # XK_grave
    126: At1Key(code=41, shift=True),  # XK_asciitilde
    65505: At1Key(code=42, shift=False),  # XK_Shift_L
    92: At1Key(code=43, shift=False),  # XK_backslash
    124: At1Key(code=43, shift=True),  # XK_bar
    90: At1Key(code=44, shift=True),  # XK_Z
    122: At1Key(code=44, shift=False),  # XK_z
    88: At1Key(code=45, shift=True),  # XK_X
    120: At1Key(code=45, shift=False),  # XK_x
    67: At1Key(code=46, shift=True),  # XK_C
    99: At1Key(code=46, shift=False),  # XK_c
    86: At1Key(code=47, shift=True),  # XK_V
    118: At1Key(code=47, shift=False),  # XK_v
    66: At1Key(code=48, shift=True),  # XK_B
    98: At1Key(code=48, shift=False),  # XK_b
    78: At1Key(code=49, shift=True),  # XK_N
    110: At1Key(code=49, shift=False),  # XK_n
    77: At1Key(code=50, shift=True),  # XK_M
    109: At1Key(code=50, shift=False),  # XK_m
    44: At1Key(code=51, shift=False),  # XK_comma
    60: At1Key(code=51, shift=True),  # XK_less
    46: At1Key(code=52, shift=False),  # XK_period
    62: At1Key(code=52, shift=True),  # XK_greater
    47: At1Key(code=53, shift=False),  # XK_slash
    63: At1Key(code=53, shift=True),  # XK_question
    65506: At1Key(code=54, shift=False),  # XK_Shift_R
    65513: At1Key(code=56, shift=False),  # XK_Alt_L
    32: At1Key(code=57, shift=False),  # XK_space
    65509: At1Key(code=58, shift=False),  # XK_Caps_Lock
    65470: At1Key(code=59, shift=False),  # XK_F1
    65471: At1Key(code=60, shift=False),  # XK_F2
    65472: At1Key(code=61, shift=False),  # XK_F3
    65473: At1Key(code=62, shift=False),  # XK_F4
    65474: At1Key(code=63, shift=False),  # XK_F5
    65475: At1Key(code=64, shift=False),  # XK_F6
    65476: At1Key(code=65, shift=False),  # XK_F7
    65477: At1Key(code=66, shift=False),  # XK_F8
    65478: At1Key(code=67, shift=False),  # XK_F9
    65479: At1Key(code=68, shift=False),  # XK_F10
    65407: At1Key(code=69, shift=False),  # XK_Num_Lock
    65300: At1Key(code=70, shift=False),  # XK_Scroll_Lock
    65301: At1Key(code=84, shift=False),  # XK_Sys_Req
    65480: At1Key(code=87, shift=False),  # XK_F11
    65481: At1Key(code=88, shift=False),  # XK_F12
    65508: At1Key(code=57373, shift=False),  # XK_Control_R
    65514: At1Key(code=57400, shift=False),  # XK_Alt_R
    65299: At1Key(code=57414, shift=False),  # XK_Pause
    65360: At1Key(code=57415, shift=False),  # XK_Home
    65362: At1Key(code=57416, shift=False),  # XK_Up
    65365: At1Key(code=57417, shift=False),  # XK_Page_Up
    65361: At1Key(code=57419, shift=False),  # XK_Left
    65363: At1Key(code=57421, shift=False),  # XK_Right
    65367: At1Key(code=57423, shift=False),  # XK_End
    65364: At1Key(code=57424, shift=False),  # XK_Down
    65366: At1Key(code=57425, shift=False),  # XK_Page_Down
    65379: At1Key(code=57426, shift=False),  # XK_Insert
    65535: At1Key(code=57427, shift=False),  # XK_Delete
    65511: At1Key(code=57435, shift=False),  # XK_Meta_L
    65512: At1Key(code=57436, shift=False),  # XK_Meta_R
    65383: At1Key(code=57437, shift=False),  # XK_Menu
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
    84: "PrintScreen",
    87: "F11",
    88: "F12",
    57373: "ControlRight",
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
}
