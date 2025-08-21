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


KEYMAP: dict[int, Key] = {
    ecodes.KEY_A: Key(mcu=McuKey(code=1), usb=UsbKey(code=4, is_modifier=False)),
    ecodes.KEY_B: Key(mcu=McuKey(code=2), usb=UsbKey(code=5, is_modifier=False)),
    ecodes.KEY_C: Key(mcu=McuKey(code=3), usb=UsbKey(code=6, is_modifier=False)),
    ecodes.KEY_D: Key(mcu=McuKey(code=4), usb=UsbKey(code=7, is_modifier=False)),
    ecodes.KEY_E: Key(mcu=McuKey(code=5), usb=UsbKey(code=8, is_modifier=False)),
    ecodes.KEY_F: Key(mcu=McuKey(code=6), usb=UsbKey(code=9, is_modifier=False)),
    ecodes.KEY_G: Key(mcu=McuKey(code=7), usb=UsbKey(code=10, is_modifier=False)),
    ecodes.KEY_H: Key(mcu=McuKey(code=8), usb=UsbKey(code=11, is_modifier=False)),
    ecodes.KEY_I: Key(mcu=McuKey(code=9), usb=UsbKey(code=12, is_modifier=False)),
    ecodes.KEY_J: Key(mcu=McuKey(code=10), usb=UsbKey(code=13, is_modifier=False)),
    ecodes.KEY_K: Key(mcu=McuKey(code=11), usb=UsbKey(code=14, is_modifier=False)),
    ecodes.KEY_L: Key(mcu=McuKey(code=12), usb=UsbKey(code=15, is_modifier=False)),
    ecodes.KEY_M: Key(mcu=McuKey(code=13), usb=UsbKey(code=16, is_modifier=False)),
    ecodes.KEY_N: Key(mcu=McuKey(code=14), usb=UsbKey(code=17, is_modifier=False)),
    ecodes.KEY_O: Key(mcu=McuKey(code=15), usb=UsbKey(code=18, is_modifier=False)),
    ecodes.KEY_P: Key(mcu=McuKey(code=16), usb=UsbKey(code=19, is_modifier=False)),
    ecodes.KEY_Q: Key(mcu=McuKey(code=17), usb=UsbKey(code=20, is_modifier=False)),
    ecodes.KEY_R: Key(mcu=McuKey(code=18), usb=UsbKey(code=21, is_modifier=False)),
    ecodes.KEY_S: Key(mcu=McuKey(code=19), usb=UsbKey(code=22, is_modifier=False)),
    ecodes.KEY_T: Key(mcu=McuKey(code=20), usb=UsbKey(code=23, is_modifier=False)),
    ecodes.KEY_U: Key(mcu=McuKey(code=21), usb=UsbKey(code=24, is_modifier=False)),
    ecodes.KEY_V: Key(mcu=McuKey(code=22), usb=UsbKey(code=25, is_modifier=False)),
    ecodes.KEY_W: Key(mcu=McuKey(code=23), usb=UsbKey(code=26, is_modifier=False)),
    ecodes.KEY_X: Key(mcu=McuKey(code=24), usb=UsbKey(code=27, is_modifier=False)),
    ecodes.KEY_Y: Key(mcu=McuKey(code=25), usb=UsbKey(code=28, is_modifier=False)),
    ecodes.KEY_Z: Key(mcu=McuKey(code=26), usb=UsbKey(code=29, is_modifier=False)),
    ecodes.KEY_1: Key(mcu=McuKey(code=27), usb=UsbKey(code=30, is_modifier=False)),
    ecodes.KEY_2: Key(mcu=McuKey(code=28), usb=UsbKey(code=31, is_modifier=False)),
    ecodes.KEY_3: Key(mcu=McuKey(code=29), usb=UsbKey(code=32, is_modifier=False)),
    ecodes.KEY_4: Key(mcu=McuKey(code=30), usb=UsbKey(code=33, is_modifier=False)),
    ecodes.KEY_5: Key(mcu=McuKey(code=31), usb=UsbKey(code=34, is_modifier=False)),
    ecodes.KEY_6: Key(mcu=McuKey(code=32), usb=UsbKey(code=35, is_modifier=False)),
    ecodes.KEY_7: Key(mcu=McuKey(code=33), usb=UsbKey(code=36, is_modifier=False)),
    ecodes.KEY_8: Key(mcu=McuKey(code=34), usb=UsbKey(code=37, is_modifier=False)),
    ecodes.KEY_9: Key(mcu=McuKey(code=35), usb=UsbKey(code=38, is_modifier=False)),
    ecodes.KEY_0: Key(mcu=McuKey(code=36), usb=UsbKey(code=39, is_modifier=False)),
    ecodes.KEY_ENTER: Key(mcu=McuKey(code=37), usb=UsbKey(code=40, is_modifier=False)),
    ecodes.KEY_ESC: Key(mcu=McuKey(code=38), usb=UsbKey(code=41, is_modifier=False)),
    ecodes.KEY_BACKSPACE: Key(mcu=McuKey(code=39), usb=UsbKey(code=42, is_modifier=False)),
    ecodes.KEY_TAB: Key(mcu=McuKey(code=40), usb=UsbKey(code=43, is_modifier=False)),
    ecodes.KEY_SPACE: Key(mcu=McuKey(code=41), usb=UsbKey(code=44, is_modifier=False)),
    ecodes.KEY_MINUS: Key(mcu=McuKey(code=42), usb=UsbKey(code=45, is_modifier=False)),
    ecodes.KEY_EQUAL: Key(mcu=McuKey(code=43), usb=UsbKey(code=46, is_modifier=False)),
    ecodes.KEY_LEFTBRACE: Key(mcu=McuKey(code=44), usb=UsbKey(code=47, is_modifier=False)),
    ecodes.KEY_RIGHTBRACE: Key(mcu=McuKey(code=45), usb=UsbKey(code=48, is_modifier=False)),
    ecodes.KEY_BACKSLASH: Key(mcu=McuKey(code=46), usb=UsbKey(code=49, is_modifier=False)),
    ecodes.KEY_SEMICOLON: Key(mcu=McuKey(code=47), usb=UsbKey(code=51, is_modifier=False)),
    ecodes.KEY_APOSTROPHE: Key(mcu=McuKey(code=48), usb=UsbKey(code=52, is_modifier=False)),
    ecodes.KEY_GRAVE: Key(mcu=McuKey(code=49), usb=UsbKey(code=53, is_modifier=False)),
    ecodes.KEY_COMMA: Key(mcu=McuKey(code=50), usb=UsbKey(code=54, is_modifier=False)),
    ecodes.KEY_DOT: Key(mcu=McuKey(code=51), usb=UsbKey(code=55, is_modifier=False)),
    ecodes.KEY_SLASH: Key(mcu=McuKey(code=52), usb=UsbKey(code=56, is_modifier=False)),
    ecodes.KEY_CAPSLOCK: Key(mcu=McuKey(code=53), usb=UsbKey(code=57, is_modifier=False)),
    ecodes.KEY_F1: Key(mcu=McuKey(code=54), usb=UsbKey(code=58, is_modifier=False)),
    ecodes.KEY_F2: Key(mcu=McuKey(code=55), usb=UsbKey(code=59, is_modifier=False)),
    ecodes.KEY_F3: Key(mcu=McuKey(code=56), usb=UsbKey(code=60, is_modifier=False)),
    ecodes.KEY_F4: Key(mcu=McuKey(code=57), usb=UsbKey(code=61, is_modifier=False)),
    ecodes.KEY_F5: Key(mcu=McuKey(code=58), usb=UsbKey(code=62, is_modifier=False)),
    ecodes.KEY_F6: Key(mcu=McuKey(code=59), usb=UsbKey(code=63, is_modifier=False)),
    ecodes.KEY_F7: Key(mcu=McuKey(code=60), usb=UsbKey(code=64, is_modifier=False)),
    ecodes.KEY_F8: Key(mcu=McuKey(code=61), usb=UsbKey(code=65, is_modifier=False)),
    ecodes.KEY_F9: Key(mcu=McuKey(code=62), usb=UsbKey(code=66, is_modifier=False)),
    ecodes.KEY_F10: Key(mcu=McuKey(code=63), usb=UsbKey(code=67, is_modifier=False)),
    ecodes.KEY_F11: Key(mcu=McuKey(code=64), usb=UsbKey(code=68, is_modifier=False)),
    ecodes.KEY_F12: Key(mcu=McuKey(code=65), usb=UsbKey(code=69, is_modifier=False)),
    ecodes.KEY_SYSRQ: Key(mcu=McuKey(code=66), usb=UsbKey(code=70, is_modifier=False)),
    ecodes.KEY_INSERT: Key(mcu=McuKey(code=67), usb=UsbKey(code=73, is_modifier=False)),
    ecodes.KEY_HOME: Key(mcu=McuKey(code=68), usb=UsbKey(code=74, is_modifier=False)),
    ecodes.KEY_PAGEUP: Key(mcu=McuKey(code=69), usb=UsbKey(code=75, is_modifier=False)),
    ecodes.KEY_DELETE: Key(mcu=McuKey(code=70), usb=UsbKey(code=76, is_modifier=False)),
    ecodes.KEY_END: Key(mcu=McuKey(code=71), usb=UsbKey(code=77, is_modifier=False)),
    ecodes.KEY_PAGEDOWN: Key(mcu=McuKey(code=72), usb=UsbKey(code=78, is_modifier=False)),
    ecodes.KEY_RIGHT: Key(mcu=McuKey(code=73), usb=UsbKey(code=79, is_modifier=False)),
    ecodes.KEY_LEFT: Key(mcu=McuKey(code=74), usb=UsbKey(code=80, is_modifier=False)),
    ecodes.KEY_DOWN: Key(mcu=McuKey(code=75), usb=UsbKey(code=81, is_modifier=False)),
    ecodes.KEY_UP: Key(mcu=McuKey(code=76), usb=UsbKey(code=82, is_modifier=False)),
    ecodes.KEY_LEFTCTRL: Key(mcu=McuKey(code=77), usb=UsbKey(code=1, is_modifier=True)),
    ecodes.KEY_LEFTSHIFT: Key(mcu=McuKey(code=78), usb=UsbKey(code=2, is_modifier=True)),
    ecodes.KEY_LEFTALT: Key(mcu=McuKey(code=79), usb=UsbKey(code=4, is_modifier=True)),
    ecodes.KEY_LEFTMETA: Key(mcu=McuKey(code=80), usb=UsbKey(code=8, is_modifier=True)),
    ecodes.KEY_RIGHTCTRL: Key(mcu=McuKey(code=81), usb=UsbKey(code=16, is_modifier=True)),
    ecodes.KEY_RIGHTSHIFT: Key(mcu=McuKey(code=82), usb=UsbKey(code=32, is_modifier=True)),
    ecodes.KEY_RIGHTALT: Key(mcu=McuKey(code=83), usb=UsbKey(code=64, is_modifier=True)),
    ecodes.KEY_RIGHTMETA: Key(mcu=McuKey(code=84), usb=UsbKey(code=128, is_modifier=True)),
    ecodes.KEY_PAUSE: Key(mcu=McuKey(code=85), usb=UsbKey(code=72, is_modifier=False)),
    ecodes.KEY_SCROLLLOCK: Key(mcu=McuKey(code=86), usb=UsbKey(code=71, is_modifier=False)),
    ecodes.KEY_NUMLOCK: Key(mcu=McuKey(code=87), usb=UsbKey(code=83, is_modifier=False)),
    ecodes.KEY_CONTEXT_MENU: Key(mcu=McuKey(code=88), usb=UsbKey(code=101, is_modifier=False)),
    ecodes.KEY_KPSLASH: Key(mcu=McuKey(code=89), usb=UsbKey(code=84, is_modifier=False)),
    ecodes.KEY_KPASTERISK: Key(mcu=McuKey(code=90), usb=UsbKey(code=85, is_modifier=False)),
    ecodes.KEY_KPMINUS: Key(mcu=McuKey(code=91), usb=UsbKey(code=86, is_modifier=False)),
    ecodes.KEY_KPPLUS: Key(mcu=McuKey(code=92), usb=UsbKey(code=87, is_modifier=False)),
    ecodes.KEY_KPENTER: Key(mcu=McuKey(code=93), usb=UsbKey(code=88, is_modifier=False)),
    ecodes.KEY_KP1: Key(mcu=McuKey(code=94), usb=UsbKey(code=89, is_modifier=False)),
    ecodes.KEY_KP2: Key(mcu=McuKey(code=95), usb=UsbKey(code=90, is_modifier=False)),
    ecodes.KEY_KP3: Key(mcu=McuKey(code=96), usb=UsbKey(code=91, is_modifier=False)),
    ecodes.KEY_KP4: Key(mcu=McuKey(code=97), usb=UsbKey(code=92, is_modifier=False)),
    ecodes.KEY_KP5: Key(mcu=McuKey(code=98), usb=UsbKey(code=93, is_modifier=False)),
    ecodes.KEY_KP6: Key(mcu=McuKey(code=99), usb=UsbKey(code=94, is_modifier=False)),
    ecodes.KEY_KP7: Key(mcu=McuKey(code=100), usb=UsbKey(code=95, is_modifier=False)),
    ecodes.KEY_KP8: Key(mcu=McuKey(code=101), usb=UsbKey(code=96, is_modifier=False)),
    ecodes.KEY_KP9: Key(mcu=McuKey(code=102), usb=UsbKey(code=97, is_modifier=False)),
    ecodes.KEY_KP0: Key(mcu=McuKey(code=103), usb=UsbKey(code=98, is_modifier=False)),
    ecodes.KEY_KPDOT: Key(mcu=McuKey(code=104), usb=UsbKey(code=99, is_modifier=False)),
    ecodes.KEY_POWER: Key(mcu=McuKey(code=105), usb=UsbKey(code=102, is_modifier=False)),
    ecodes.KEY_102ND: Key(mcu=McuKey(code=106), usb=UsbKey(code=100, is_modifier=False)),
    ecodes.KEY_YEN: Key(mcu=McuKey(code=107), usb=UsbKey(code=137, is_modifier=False)),
    ecodes.KEY_RO: Key(mcu=McuKey(code=108), usb=UsbKey(code=135, is_modifier=False)),
    ecodes.KEY_KATAKANA: Key(mcu=McuKey(code=109), usb=UsbKey(code=136, is_modifier=False)),
    ecodes.KEY_HENKAN: Key(mcu=McuKey(code=110), usb=UsbKey(code=138, is_modifier=False)),
    ecodes.KEY_MUHENKAN: Key(mcu=McuKey(code=111), usb=UsbKey(code=139, is_modifier=False)),
    ecodes.KEY_MUTE: Key(mcu=McuKey(code=112), usb=UsbKey(code=127, is_modifier=False)),
    ecodes.KEY_VOLUMEUP: Key(mcu=McuKey(code=113), usb=UsbKey(code=128, is_modifier=False)),
    ecodes.KEY_VOLUMEDOWN: Key(mcu=McuKey(code=114), usb=UsbKey(code=129, is_modifier=False)),
    ecodes.KEY_F20: Key(mcu=McuKey(code=115), usb=UsbKey(code=111, is_modifier=False)),
}


WEB_TO_EVDEV = {
    "KeyA": ecodes.KEY_A,
    "KeyB": ecodes.KEY_B,
    "KeyC": ecodes.KEY_C,
    "KeyD": ecodes.KEY_D,
    "KeyE": ecodes.KEY_E,
    "KeyF": ecodes.KEY_F,
    "KeyG": ecodes.KEY_G,
    "KeyH": ecodes.KEY_H,
    "KeyI": ecodes.KEY_I,
    "KeyJ": ecodes.KEY_J,
    "KeyK": ecodes.KEY_K,
    "KeyL": ecodes.KEY_L,
    "KeyM": ecodes.KEY_M,
    "KeyN": ecodes.KEY_N,
    "KeyO": ecodes.KEY_O,
    "KeyP": ecodes.KEY_P,
    "KeyQ": ecodes.KEY_Q,
    "KeyR": ecodes.KEY_R,
    "KeyS": ecodes.KEY_S,
    "KeyT": ecodes.KEY_T,
    "KeyU": ecodes.KEY_U,
    "KeyV": ecodes.KEY_V,
    "KeyW": ecodes.KEY_W,
    "KeyX": ecodes.KEY_X,
    "KeyY": ecodes.KEY_Y,
    "KeyZ": ecodes.KEY_Z,
    "Digit1": ecodes.KEY_1,
    "Digit2": ecodes.KEY_2,
    "Digit3": ecodes.KEY_3,
    "Digit4": ecodes.KEY_4,
    "Digit5": ecodes.KEY_5,
    "Digit6": ecodes.KEY_6,
    "Digit7": ecodes.KEY_7,
    "Digit8": ecodes.KEY_8,
    "Digit9": ecodes.KEY_9,
    "Digit0": ecodes.KEY_0,
    "Enter": ecodes.KEY_ENTER,
    "Escape": ecodes.KEY_ESC,
    "Backspace": ecodes.KEY_BACKSPACE,
    "Tab": ecodes.KEY_TAB,
    "Space": ecodes.KEY_SPACE,
    "Minus": ecodes.KEY_MINUS,
    "Equal": ecodes.KEY_EQUAL,
    "BracketLeft": ecodes.KEY_LEFTBRACE,
    "BracketRight": ecodes.KEY_RIGHTBRACE,
    "Backslash": ecodes.KEY_BACKSLASH,
    "Semicolon": ecodes.KEY_SEMICOLON,
    "Quote": ecodes.KEY_APOSTROPHE,
    "Backquote": ecodes.KEY_GRAVE,
    "Comma": ecodes.KEY_COMMA,
    "Period": ecodes.KEY_DOT,
    "Slash": ecodes.KEY_SLASH,
    "CapsLock": ecodes.KEY_CAPSLOCK,
    "F1": ecodes.KEY_F1,
    "F2": ecodes.KEY_F2,
    "F3": ecodes.KEY_F3,
    "F4": ecodes.KEY_F4,
    "F5": ecodes.KEY_F5,
    "F6": ecodes.KEY_F6,
    "F7": ecodes.KEY_F7,
    "F8": ecodes.KEY_F8,
    "F9": ecodes.KEY_F9,
    "F10": ecodes.KEY_F10,
    "F11": ecodes.KEY_F11,
    "F12": ecodes.KEY_F12,
    "PrintScreen": ecodes.KEY_SYSRQ,
    "Insert": ecodes.KEY_INSERT,
    "Home": ecodes.KEY_HOME,
    "PageUp": ecodes.KEY_PAGEUP,
    "Delete": ecodes.KEY_DELETE,
    "End": ecodes.KEY_END,
    "PageDown": ecodes.KEY_PAGEDOWN,
    "ArrowRight": ecodes.KEY_RIGHT,
    "ArrowLeft": ecodes.KEY_LEFT,
    "ArrowDown": ecodes.KEY_DOWN,
    "ArrowUp": ecodes.KEY_UP,
    "ControlLeft": ecodes.KEY_LEFTCTRL,
    "ShiftLeft": ecodes.KEY_LEFTSHIFT,
    "AltLeft": ecodes.KEY_LEFTALT,
    "MetaLeft": ecodes.KEY_LEFTMETA,
    "ControlRight": ecodes.KEY_RIGHTCTRL,
    "ShiftRight": ecodes.KEY_RIGHTSHIFT,
    "AltRight": ecodes.KEY_RIGHTALT,
    "MetaRight": ecodes.KEY_RIGHTMETA,
    "Pause": ecodes.KEY_PAUSE,
    "ScrollLock": ecodes.KEY_SCROLLLOCK,
    "NumLock": ecodes.KEY_NUMLOCK,
    "ContextMenu": ecodes.KEY_CONTEXT_MENU,
    "NumpadDivide": ecodes.KEY_KPSLASH,
    "NumpadMultiply": ecodes.KEY_KPASTERISK,
    "NumpadSubtract": ecodes.KEY_KPMINUS,
    "NumpadAdd": ecodes.KEY_KPPLUS,
    "NumpadEnter": ecodes.KEY_KPENTER,
    "Numpad1": ecodes.KEY_KP1,
    "Numpad2": ecodes.KEY_KP2,
    "Numpad3": ecodes.KEY_KP3,
    "Numpad4": ecodes.KEY_KP4,
    "Numpad5": ecodes.KEY_KP5,
    "Numpad6": ecodes.KEY_KP6,
    "Numpad7": ecodes.KEY_KP7,
    "Numpad8": ecodes.KEY_KP8,
    "Numpad9": ecodes.KEY_KP9,
    "Numpad0": ecodes.KEY_KP0,
    "NumpadDecimal": ecodes.KEY_KPDOT,
    "Power": ecodes.KEY_POWER,
    "IntlBackslash": ecodes.KEY_102ND,
    "IntlYen": ecodes.KEY_YEN,
    "IntlRo": ecodes.KEY_RO,
    "KanaMode": ecodes.KEY_KATAKANA,
    "Convert": ecodes.KEY_HENKAN,
    "NonConvert": ecodes.KEY_MUHENKAN,
    "AudioVolumeMute": ecodes.KEY_MUTE,
    "AudioVolumeUp": ecodes.KEY_VOLUMEUP,
    "AudioVolumeDown": ecodes.KEY_VOLUMEDOWN,
    "F20": ecodes.KEY_F20,
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


AT1_TO_EVDEV = {
    1: ecodes.KEY_ESC,
    2: ecodes.KEY_1,
    3: ecodes.KEY_2,
    4: ecodes.KEY_3,
    5: ecodes.KEY_4,
    6: ecodes.KEY_5,
    7: ecodes.KEY_6,
    8: ecodes.KEY_7,
    9: ecodes.KEY_8,
    10: ecodes.KEY_9,
    11: ecodes.KEY_0,
    12: ecodes.KEY_MINUS,
    13: ecodes.KEY_EQUAL,
    14: ecodes.KEY_BACKSPACE,
    15: ecodes.KEY_TAB,
    16: ecodes.KEY_Q,
    17: ecodes.KEY_W,
    18: ecodes.KEY_E,
    19: ecodes.KEY_R,
    20: ecodes.KEY_T,
    21: ecodes.KEY_Y,
    22: ecodes.KEY_U,
    23: ecodes.KEY_I,
    24: ecodes.KEY_O,
    25: ecodes.KEY_P,
    26: ecodes.KEY_LEFTBRACE,
    27: ecodes.KEY_RIGHTBRACE,
    28: ecodes.KEY_ENTER,
    29: ecodes.KEY_LEFTCTRL,
    30: ecodes.KEY_A,
    31: ecodes.KEY_S,
    32: ecodes.KEY_D,
    33: ecodes.KEY_F,
    34: ecodes.KEY_G,
    35: ecodes.KEY_H,
    36: ecodes.KEY_J,
    37: ecodes.KEY_K,
    38: ecodes.KEY_L,
    39: ecodes.KEY_SEMICOLON,
    40: ecodes.KEY_APOSTROPHE,
    41: ecodes.KEY_GRAVE,
    42: ecodes.KEY_LEFTSHIFT,
    43: ecodes.KEY_BACKSLASH,
    44: ecodes.KEY_Z,
    45: ecodes.KEY_X,
    46: ecodes.KEY_C,
    47: ecodes.KEY_V,
    48: ecodes.KEY_B,
    49: ecodes.KEY_N,
    50: ecodes.KEY_M,
    51: ecodes.KEY_COMMA,
    52: ecodes.KEY_DOT,
    53: ecodes.KEY_SLASH,
    54: ecodes.KEY_RIGHTSHIFT,
    55: ecodes.KEY_KPASTERISK,
    56: ecodes.KEY_LEFTALT,
    57: ecodes.KEY_SPACE,
    58: ecodes.KEY_CAPSLOCK,
    59: ecodes.KEY_F1,
    60: ecodes.KEY_F2,
    61: ecodes.KEY_F3,
    62: ecodes.KEY_F4,
    63: ecodes.KEY_F5,
    64: ecodes.KEY_F6,
    65: ecodes.KEY_F7,
    66: ecodes.KEY_F8,
    67: ecodes.KEY_F9,
    68: ecodes.KEY_F10,
    69: ecodes.KEY_NUMLOCK,
    70: ecodes.KEY_SCROLLLOCK,
    71: ecodes.KEY_KP7,
    72: ecodes.KEY_KP8,
    73: ecodes.KEY_KP9,
    74: ecodes.KEY_KPMINUS,
    75: ecodes.KEY_KP4,
    76: ecodes.KEY_KP5,
    77: ecodes.KEY_KP6,
    78: ecodes.KEY_KPPLUS,
    79: ecodes.KEY_KP1,
    80: ecodes.KEY_KP2,
    81: ecodes.KEY_KP3,
    82: ecodes.KEY_KP0,
    83: ecodes.KEY_KPDOT,
    84: ecodes.KEY_SYSRQ,
    86: ecodes.KEY_102ND,
    87: ecodes.KEY_F11,
    88: ecodes.KEY_F12,
    90: ecodes.KEY_F20,
    112: ecodes.KEY_KATAKANA,
    115: ecodes.KEY_RO,
    121: ecodes.KEY_HENKAN,
    123: ecodes.KEY_MUHENKAN,
    125: ecodes.KEY_YEN,
    57372: ecodes.KEY_KPENTER,
    57373: ecodes.KEY_RIGHTCTRL,
    57376: ecodes.KEY_MUTE,
    57390: ecodes.KEY_VOLUMEDOWN,
    57392: ecodes.KEY_VOLUMEUP,
    57397: ecodes.KEY_KPSLASH,
    57400: ecodes.KEY_RIGHTALT,
    57414: ecodes.KEY_PAUSE,
    57415: ecodes.KEY_HOME,
    57416: ecodes.KEY_UP,
    57417: ecodes.KEY_PAGEUP,
    57419: ecodes.KEY_LEFT,
    57421: ecodes.KEY_RIGHT,
    57423: ecodes.KEY_END,
    57424: ecodes.KEY_DOWN,
    57425: ecodes.KEY_PAGEDOWN,
    57426: ecodes.KEY_INSERT,
    57427: ecodes.KEY_DELETE,
    57435: ecodes.KEY_LEFTMETA,
    57436: ecodes.KEY_RIGHTMETA,
    57437: ecodes.KEY_CONTEXT_MENU,
    57438: ecodes.KEY_POWER,
}
