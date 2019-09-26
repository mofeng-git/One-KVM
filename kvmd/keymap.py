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
class Key:
    serial: SerialKey


# =====
KEYMAP: Dict[str, Key] = {
    "AltLeft": Key(serial=SerialKey(code=79)),
    "AltRight": Key(serial=SerialKey(code=83)),
    "ArrowDown": Key(serial=SerialKey(code=75)),
    "ArrowLeft": Key(serial=SerialKey(code=74)),
    "ArrowRight": Key(serial=SerialKey(code=73)),
    "ArrowUp": Key(serial=SerialKey(code=76)),
    "Backquote": Key(serial=SerialKey(code=49)),
    "Backslash": Key(serial=SerialKey(code=46)),
    "Backspace": Key(serial=SerialKey(code=39)),
    "BracketLeft": Key(serial=SerialKey(code=44)),
    "BracketRight": Key(serial=SerialKey(code=45)),
    "CapsLock": Key(serial=SerialKey(code=53)),
    "Comma": Key(serial=SerialKey(code=50)),
    "ControlLeft": Key(serial=SerialKey(code=77)),
    "ControlRight": Key(serial=SerialKey(code=81)),
    "Delete": Key(serial=SerialKey(code=70)),
    "Digit0": Key(serial=SerialKey(code=36)),
    "Digit1": Key(serial=SerialKey(code=27)),
    "Digit2": Key(serial=SerialKey(code=28)),
    "Digit3": Key(serial=SerialKey(code=29)),
    "Digit4": Key(serial=SerialKey(code=30)),
    "Digit5": Key(serial=SerialKey(code=31)),
    "Digit6": Key(serial=SerialKey(code=32)),
    "Digit7": Key(serial=SerialKey(code=33)),
    "Digit8": Key(serial=SerialKey(code=34)),
    "Digit9": Key(serial=SerialKey(code=35)),
    "End": Key(serial=SerialKey(code=71)),
    "Enter": Key(serial=SerialKey(code=37)),
    "Equal": Key(serial=SerialKey(code=43)),
    "Escape": Key(serial=SerialKey(code=38)),
    "F1": Key(serial=SerialKey(code=54)),
    "F10": Key(serial=SerialKey(code=63)),
    "F11": Key(serial=SerialKey(code=64)),
    "F12": Key(serial=SerialKey(code=65)),
    "F2": Key(serial=SerialKey(code=55)),
    "F3": Key(serial=SerialKey(code=56)),
    "F4": Key(serial=SerialKey(code=57)),
    "F5": Key(serial=SerialKey(code=58)),
    "F6": Key(serial=SerialKey(code=59)),
    "F7": Key(serial=SerialKey(code=60)),
    "F8": Key(serial=SerialKey(code=61)),
    "F9": Key(serial=SerialKey(code=62)),
    "Home": Key(serial=SerialKey(code=68)),
    "Insert": Key(serial=SerialKey(code=67)),
    "KeyA": Key(serial=SerialKey(code=1)),
    "KeyB": Key(serial=SerialKey(code=2)),
    "KeyC": Key(serial=SerialKey(code=3)),
    "KeyD": Key(serial=SerialKey(code=4)),
    "KeyE": Key(serial=SerialKey(code=5)),
    "KeyF": Key(serial=SerialKey(code=6)),
    "KeyG": Key(serial=SerialKey(code=7)),
    "KeyH": Key(serial=SerialKey(code=8)),
    "KeyI": Key(serial=SerialKey(code=9)),
    "KeyJ": Key(serial=SerialKey(code=10)),
    "KeyK": Key(serial=SerialKey(code=11)),
    "KeyL": Key(serial=SerialKey(code=12)),
    "KeyM": Key(serial=SerialKey(code=13)),
    "KeyN": Key(serial=SerialKey(code=14)),
    "KeyO": Key(serial=SerialKey(code=15)),
    "KeyP": Key(serial=SerialKey(code=16)),
    "KeyQ": Key(serial=SerialKey(code=17)),
    "KeyR": Key(serial=SerialKey(code=18)),
    "KeyS": Key(serial=SerialKey(code=19)),
    "KeyT": Key(serial=SerialKey(code=20)),
    "KeyU": Key(serial=SerialKey(code=21)),
    "KeyV": Key(serial=SerialKey(code=22)),
    "KeyW": Key(serial=SerialKey(code=23)),
    "KeyX": Key(serial=SerialKey(code=24)),
    "KeyY": Key(serial=SerialKey(code=25)),
    "KeyZ": Key(serial=SerialKey(code=26)),
    "MetaLeft": Key(serial=SerialKey(code=80)),
    "MetaRight": Key(serial=SerialKey(code=84)),
    "Minus": Key(serial=SerialKey(code=42)),
    "PageDown": Key(serial=SerialKey(code=72)),
    "PageUp": Key(serial=SerialKey(code=69)),
    "Pause": Key(serial=SerialKey(code=85)),
    "Period": Key(serial=SerialKey(code=51)),
    "PrintScreen": Key(serial=SerialKey(code=66)),
    "Quote": Key(serial=SerialKey(code=48)),
    "ScrollLock": Key(serial=SerialKey(code=86)),
    "Semicolon": Key(serial=SerialKey(code=47)),
    "ShiftLeft": Key(serial=SerialKey(code=78)),
    "ShiftRight": Key(serial=SerialKey(code=82)),
    "Slash": Key(serial=SerialKey(code=52)),
    "Space": Key(serial=SerialKey(code=41)),
    "Tab": Key(serial=SerialKey(code=40)),
}
