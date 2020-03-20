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


# =====
KEYMAP: Dict[str, Key] = {
    "KeyA": Key(
        serial=SerialKey(code=1),
        otg=OtgKey(code=4, is_modifier=False),
    ),
    "KeyB": Key(
        serial=SerialKey(code=2),
        otg=OtgKey(code=5, is_modifier=False),
    ),
    "KeyC": Key(
        serial=SerialKey(code=3),
        otg=OtgKey(code=6, is_modifier=False),
    ),
    "KeyD": Key(
        serial=SerialKey(code=4),
        otg=OtgKey(code=7, is_modifier=False),
    ),
    "KeyE": Key(
        serial=SerialKey(code=5),
        otg=OtgKey(code=8, is_modifier=False),
    ),
    "KeyF": Key(
        serial=SerialKey(code=6),
        otg=OtgKey(code=9, is_modifier=False),
    ),
    "KeyG": Key(
        serial=SerialKey(code=7),
        otg=OtgKey(code=10, is_modifier=False),
    ),
    "KeyH": Key(
        serial=SerialKey(code=8),
        otg=OtgKey(code=11, is_modifier=False),
    ),
    "KeyI": Key(
        serial=SerialKey(code=9),
        otg=OtgKey(code=12, is_modifier=False),
    ),
    "KeyJ": Key(
        serial=SerialKey(code=10),
        otg=OtgKey(code=13, is_modifier=False),
    ),
    "KeyK": Key(
        serial=SerialKey(code=11),
        otg=OtgKey(code=14, is_modifier=False),
    ),
    "KeyL": Key(
        serial=SerialKey(code=12),
        otg=OtgKey(code=15, is_modifier=False),
    ),
    "KeyM": Key(
        serial=SerialKey(code=13),
        otg=OtgKey(code=16, is_modifier=False),
    ),
    "KeyN": Key(
        serial=SerialKey(code=14),
        otg=OtgKey(code=17, is_modifier=False),
    ),
    "KeyO": Key(
        serial=SerialKey(code=15),
        otg=OtgKey(code=18, is_modifier=False),
    ),
    "KeyP": Key(
        serial=SerialKey(code=16),
        otg=OtgKey(code=19, is_modifier=False),
    ),
    "KeyQ": Key(
        serial=SerialKey(code=17),
        otg=OtgKey(code=20, is_modifier=False),
    ),
    "KeyR": Key(
        serial=SerialKey(code=18),
        otg=OtgKey(code=21, is_modifier=False),
    ),
    "KeyS": Key(
        serial=SerialKey(code=19),
        otg=OtgKey(code=22, is_modifier=False),
    ),
    "KeyT": Key(
        serial=SerialKey(code=20),
        otg=OtgKey(code=23, is_modifier=False),
    ),
    "KeyU": Key(
        serial=SerialKey(code=21),
        otg=OtgKey(code=24, is_modifier=False),
    ),
    "KeyV": Key(
        serial=SerialKey(code=22),
        otg=OtgKey(code=25, is_modifier=False),
    ),
    "KeyW": Key(
        serial=SerialKey(code=23),
        otg=OtgKey(code=26, is_modifier=False),
    ),
    "KeyX": Key(
        serial=SerialKey(code=24),
        otg=OtgKey(code=27, is_modifier=False),
    ),
    "KeyY": Key(
        serial=SerialKey(code=25),
        otg=OtgKey(code=28, is_modifier=False),
    ),
    "KeyZ": Key(
        serial=SerialKey(code=26),
        otg=OtgKey(code=29, is_modifier=False),
    ),
    "Digit1": Key(
        serial=SerialKey(code=27),
        otg=OtgKey(code=30, is_modifier=False),
    ),
    "Digit2": Key(
        serial=SerialKey(code=28),
        otg=OtgKey(code=31, is_modifier=False),
    ),
    "Digit3": Key(
        serial=SerialKey(code=29),
        otg=OtgKey(code=32, is_modifier=False),
    ),
    "Digit4": Key(
        serial=SerialKey(code=30),
        otg=OtgKey(code=33, is_modifier=False),
    ),
    "Digit5": Key(
        serial=SerialKey(code=31),
        otg=OtgKey(code=34, is_modifier=False),
    ),
    "Digit6": Key(
        serial=SerialKey(code=32),
        otg=OtgKey(code=35, is_modifier=False),
    ),
    "Digit7": Key(
        serial=SerialKey(code=33),
        otg=OtgKey(code=36, is_modifier=False),
    ),
    "Digit8": Key(
        serial=SerialKey(code=34),
        otg=OtgKey(code=37, is_modifier=False),
    ),
    "Digit9": Key(
        serial=SerialKey(code=35),
        otg=OtgKey(code=38, is_modifier=False),
    ),
    "Digit0": Key(
        serial=SerialKey(code=36),
        otg=OtgKey(code=39, is_modifier=False),
    ),
    "Enter": Key(
        serial=SerialKey(code=37),
        otg=OtgKey(code=40, is_modifier=False),
    ),
    "Escape": Key(
        serial=SerialKey(code=38),
        otg=OtgKey(code=41, is_modifier=False),
    ),
    "Backspace": Key(
        serial=SerialKey(code=39),
        otg=OtgKey(code=42, is_modifier=False),
    ),
    "Tab": Key(
        serial=SerialKey(code=40),
        otg=OtgKey(code=43, is_modifier=False),
    ),
    "Space": Key(
        serial=SerialKey(code=41),
        otg=OtgKey(code=44, is_modifier=False),
    ),
    "Minus": Key(
        serial=SerialKey(code=42),
        otg=OtgKey(code=45, is_modifier=False),
    ),
    "Equal": Key(
        serial=SerialKey(code=43),
        otg=OtgKey(code=46, is_modifier=False),
    ),
    "BracketLeft": Key(
        serial=SerialKey(code=44),
        otg=OtgKey(code=47, is_modifier=False),
    ),
    "BracketRight": Key(
        serial=SerialKey(code=45),
        otg=OtgKey(code=48, is_modifier=False),
    ),
    "Backslash": Key(
        serial=SerialKey(code=46),
        otg=OtgKey(code=49, is_modifier=False),
    ),
    "Semicolon": Key(
        serial=SerialKey(code=47),
        otg=OtgKey(code=51, is_modifier=False),
    ),
    "Quote": Key(
        serial=SerialKey(code=48),
        otg=OtgKey(code=52, is_modifier=False),
    ),
    "Backquote": Key(
        serial=SerialKey(code=49),
        otg=OtgKey(code=53, is_modifier=False),
    ),
    "Comma": Key(
        serial=SerialKey(code=50),
        otg=OtgKey(code=54, is_modifier=False),
    ),
    "Period": Key(
        serial=SerialKey(code=51),
        otg=OtgKey(code=55, is_modifier=False),
    ),
    "Slash": Key(
        serial=SerialKey(code=52),
        otg=OtgKey(code=56, is_modifier=False),
    ),
    "CapsLock": Key(
        serial=SerialKey(code=53),
        otg=OtgKey(code=57, is_modifier=False),
    ),
    "F1": Key(
        serial=SerialKey(code=54),
        otg=OtgKey(code=58, is_modifier=False),
    ),
    "F2": Key(
        serial=SerialKey(code=55),
        otg=OtgKey(code=59, is_modifier=False),
    ),
    "F3": Key(
        serial=SerialKey(code=56),
        otg=OtgKey(code=60, is_modifier=False),
    ),
    "F4": Key(
        serial=SerialKey(code=57),
        otg=OtgKey(code=61, is_modifier=False),
    ),
    "F5": Key(
        serial=SerialKey(code=58),
        otg=OtgKey(code=62, is_modifier=False),
    ),
    "F6": Key(
        serial=SerialKey(code=59),
        otg=OtgKey(code=63, is_modifier=False),
    ),
    "F7": Key(
        serial=SerialKey(code=60),
        otg=OtgKey(code=64, is_modifier=False),
    ),
    "F8": Key(
        serial=SerialKey(code=61),
        otg=OtgKey(code=65, is_modifier=False),
    ),
    "F9": Key(
        serial=SerialKey(code=62),
        otg=OtgKey(code=66, is_modifier=False),
    ),
    "F10": Key(
        serial=SerialKey(code=63),
        otg=OtgKey(code=67, is_modifier=False),
    ),
    "F11": Key(
        serial=SerialKey(code=64),
        otg=OtgKey(code=68, is_modifier=False),
    ),
    "F12": Key(
        serial=SerialKey(code=65),
        otg=OtgKey(code=69, is_modifier=False),
    ),
    "PrintScreen": Key(
        serial=SerialKey(code=66),
        otg=OtgKey(code=70, is_modifier=False),
    ),
    "Insert": Key(
        serial=SerialKey(code=67),
        otg=OtgKey(code=73, is_modifier=False),
    ),
    "Home": Key(
        serial=SerialKey(code=68),
        otg=OtgKey(code=74, is_modifier=False),
    ),
    "PageUp": Key(
        serial=SerialKey(code=69),
        otg=OtgKey(code=75, is_modifier=False),
    ),
    "Delete": Key(
        serial=SerialKey(code=70),
        otg=OtgKey(code=76, is_modifier=False),
    ),
    "End": Key(
        serial=SerialKey(code=71),
        otg=OtgKey(code=77, is_modifier=False),
    ),
    "PageDown": Key(
        serial=SerialKey(code=72),
        otg=OtgKey(code=78, is_modifier=False),
    ),
    "ArrowRight": Key(
        serial=SerialKey(code=73),
        otg=OtgKey(code=79, is_modifier=False),
    ),
    "ArrowLeft": Key(
        serial=SerialKey(code=74),
        otg=OtgKey(code=80, is_modifier=False),
    ),
    "ArrowDown": Key(
        serial=SerialKey(code=75),
        otg=OtgKey(code=81, is_modifier=False),
    ),
    "ArrowUp": Key(
        serial=SerialKey(code=76),
        otg=OtgKey(code=82, is_modifier=False),
    ),
    "ControlLeft": Key(
        serial=SerialKey(code=77),
        otg=OtgKey(code=1, is_modifier=True),
    ),
    "ShiftLeft": Key(
        serial=SerialKey(code=78),
        otg=OtgKey(code=2, is_modifier=True),
    ),
    "AltLeft": Key(
        serial=SerialKey(code=79),
        otg=OtgKey(code=4, is_modifier=True),
    ),
    "MetaLeft": Key(
        serial=SerialKey(code=80),
        otg=OtgKey(code=8, is_modifier=True),
    ),
    "ControlRight": Key(
        serial=SerialKey(code=81),
        otg=OtgKey(code=16, is_modifier=True),
    ),
    "ShiftRight": Key(
        serial=SerialKey(code=82),
        otg=OtgKey(code=32, is_modifier=True),
    ),
    "AltRight": Key(
        serial=SerialKey(code=83),
        otg=OtgKey(code=64, is_modifier=True),
    ),
    "MetaRight": Key(
        serial=SerialKey(code=84),
        otg=OtgKey(code=128, is_modifier=True),
    ),
    "Pause": Key(
        serial=SerialKey(code=85),
        otg=OtgKey(code=72, is_modifier=False),
    ),
    "ScrollLock": Key(
        serial=SerialKey(code=86),
        otg=OtgKey(code=71, is_modifier=False),
    ),
    "NumLock": Key(
        serial=SerialKey(code=87),
        otg=OtgKey(code=83, is_modifier=False),
    ),
    "ContextMenu": Key(
        serial=SerialKey(code=88),
        otg=OtgKey(code=101, is_modifier=False),
    ),
}


# =====
X11_TO_AT1 = {
    65307: 1,
    33: 2,
    49: 2,
    50: 3,
    64: 3,
    35: 4,
    51: 4,
    36: 5,
    52: 5,
    37: 6,
    53: 6,
    54: 7,
    94: 7,
    38: 8,
    55: 8,
    42: 9,
    56: 9,
    40: 10,
    57: 10,
    41: 11,
    48: 11,
    45: 12,
    95: 12,
    43: 13,
    61: 13,
    65288: 14,
    65289: 15,
    81: 16,
    113: 16,
    87: 17,
    119: 17,
    69: 18,
    101: 18,
    82: 19,
    114: 19,
    84: 20,
    116: 20,
    89: 21,
    121: 21,
    85: 22,
    117: 22,
    73: 23,
    105: 23,
    79: 24,
    111: 24,
    80: 25,
    112: 25,
    91: 26,
    123: 26,
    93: 27,
    125: 27,
    65293: 28,
    65507: 29,
    65: 30,
    97: 30,
    83: 31,
    115: 31,
    68: 32,
    100: 32,
    70: 33,
    102: 33,
    71: 34,
    103: 34,
    72: 35,
    104: 35,
    74: 36,
    106: 36,
    75: 37,
    107: 37,
    76: 38,
    108: 38,
    58: 39,
    59: 39,
    34: 40,
    39: 40,
    96: 41,
    126: 41,
    65505: 42,
    92: 43,
    124: 43,
    90: 44,
    122: 44,
    88: 45,
    120: 45,
    67: 46,
    99: 46,
    86: 47,
    118: 47,
    66: 48,
    98: 48,
    78: 49,
    110: 49,
    77: 50,
    109: 50,
    44: 51,
    60: 51,
    46: 52,
    62: 52,
    47: 53,
    63: 53,
    65506: 54,
    65513: 56,
    32: 57,
    65509: 58,
    65470: 59,
    65471: 60,
    65472: 61,
    65473: 62,
    65474: 63,
    65475: 64,
    65476: 65,
    65477: 66,
    65478: 67,
    65479: 68,
    65407: 69,
    65300: 70,
    65301: 84,
    65480: 87,
    65481: 88,
    65508: 57373,
    65514: 57400,
    65299: 57414,
    65360: 57415,
    65362: 57416,
    65365: 57417,
    65361: 57419,
    65363: 57421,
    65367: 57423,
    65364: 57424,
    65366: 57425,
    65379: 57426,
    65535: 57427,
    65511: 57435,
    65512: 57436,
    65383: 57437,
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
