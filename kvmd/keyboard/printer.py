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


import string

from typing import Tuple
from typing import Generator

from .mappings import KEYMAP


# =====
_LOWER_CHARS = {
    "\n": "Enter",
    "\t": "Tab",
    " ": "Space",
    "`": "Backquote",
    "\\": "Backslash",
    "[": "BracketLeft",
    "]": "BracketLeft",
    ",": "Comma",
    ".": "Period",
    "-": "Minus",
    "'": "Quote",
    ";": "Semicolon",
    "/": "Slash",
    "=": "Equal",
    **{str(number): f"Digit{number}" for number in range(0, 10)},
    **{ch: f"Key{ch.upper()}" for ch in string.ascii_lowercase},
}
assert not set(_LOWER_CHARS.values()).difference(KEYMAP)

_UPPER_CHARS = {
    "~": "Backquote",
    "|": "Backslash",
    "{": "BracketLeft",
    "}": "BracketRight",
    "<": "Comma",
    ">": "Period",
    "!": "Digit1",
    "@": "Digit2",
    "#": "Digit3",
    "$": "Digit4",
    "%": "Digit5",
    "^": "Digit6",
    "&": "Digit7",
    "*": "Digit8",
    "(": "Digit9",
    ")": "Digit0",
    "_": "Minus",
    "\"": "Quote",
    ":": "Semicolon",
    "?": "Slash",
    "+": "Equal",
    **{ch: f"Key{ch}" for ch in string.ascii_uppercase},
}
assert not set(_UPPER_CHARS.values()).difference(KEYMAP)


# =====
def text_to_web_keys(text: str, shift_key: str="ShiftLeft") -> Generator[Tuple[str, bool], None, None]:
    assert shift_key in ["ShiftLeft", "ShiftRight"]

    shifted = False
    for ch in text:
        upper = False
        key = _LOWER_CHARS.get(ch)
        if key is None:
            if (key := _UPPER_CHARS.get(ch)) is None:
                continue
            upper = True

        if upper and not shifted:
            yield (shift_key, True)
            shifted = True
        elif not upper and shifted:
            yield (shift_key, False)
            shifted = False

        yield (key, True)
        yield (key, False)

    if shifted:
        yield (shift_key, False)
