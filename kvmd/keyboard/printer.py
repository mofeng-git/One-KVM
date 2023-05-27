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


import ctypes
import ctypes.util

from typing import Generator

from .keysym import SymmapModifiers
from .mappings import WebModifiers


# =====
def _load_libxkbcommon() -> ctypes.CDLL:
    path = ctypes.util.find_library("xkbcommon")
    if not path:
        raise RuntimeError("Where is libxkbcommon?")
    assert path
    lib = ctypes.CDLL(path)
    for (name, restype, argtypes) in [
        ("xkb_utf32_to_keysym", ctypes.c_uint32, [ctypes.c_uint32]),
    ]:
        func = getattr(lib, name)
        if not func:
            raise RuntimeError(f"Where is libc.{name}?")
        setattr(func, "restype", restype)
        setattr(func, "argtypes", argtypes)
    return lib


_libxkbcommon = _load_libxkbcommon()


def _ch_to_keysym(ch: str) -> int:
    assert len(ch) == 1
    return _libxkbcommon.xkb_utf32_to_keysym(ord(ch))


# =====
def text_to_web_keys(  # pylint: disable=too-many-branches
    text: str,
    symmap: dict[int, dict[int, str]],
) -> Generator[tuple[str, bool], None, None]:

    shift = False
    altgr = False

    for ch in text:
        # https://stackoverflow.com/questions/12343987/convert-ascii-character-to-x11-keycode
        # https://www.ascii-code.com
        if ch == "\n":
            keys = {0: "Enter"}
        elif ch == "\t":
            keys = {0: "Tab"}
        elif ch == " ":
            keys = {0: "Space"}
        else:
            if ch in ["‚", "‘", "’"]:
                ch = "'"
            elif ch in ["„", "“", "”"]:
                ch = "\""
            elif ch == "–":  # Short
                ch = "-"
            elif ch == "—":  # Long
                ch = "--"
            if not ch.isprintable():
                continue
            try:
                keys = symmap[_ch_to_keysym(ch)]
            except Exception:
                continue

        for (modifiers, key) in keys.items():
            if modifiers & SymmapModifiers.CTRL:
                # Not supported yet
                continue

            if modifiers & SymmapModifiers.SHIFT and not shift:
                yield (WebModifiers.SHIFT_LEFT, True)
                shift = True
            elif not (modifiers & SymmapModifiers.SHIFT) and shift:
                yield (WebModifiers.SHIFT_LEFT, False)
                shift = False

            if modifiers & SymmapModifiers.ALTGR and not altgr:
                yield (WebModifiers.ALT_RIGHT, True)
                altgr = True
            elif not (modifiers & SymmapModifiers.ALTGR) and altgr:
                yield (WebModifiers.ALT_RIGHT, False)
                altgr = False

            yield (key, True)
            yield (key, False)
            break

    if shift:
        yield (WebModifiers.SHIFT_LEFT, False)
    if altgr:
        yield (WebModifiers.ALT_RIGHT, False)
