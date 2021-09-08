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


from typing import Tuple
from typing import Dict
from typing import Generator

from .keysym import SymmapModifiers
from .mappings import WebModifiers


# =====
def text_to_web_keys(  # pylint: disable=too-many-branches
    text: str,
    symmap: Dict[int, Dict[int, str]],
    shift_key: str=WebModifiers.SHIFT_LEFT,
) -> Generator[Tuple[str, bool], None, None]:

    assert shift_key in WebModifiers.SHIFTS

    shifted = False
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
            if not ch.isprintable():
                continue
            try:
                keys = symmap[ord(ch)]
            except Exception:
                continue

        for (modifiers, key) in reversed(keys.items()):
            if (modifiers & SymmapModifiers.ALTGR) or (modifiers & SymmapModifiers.CTRL):
                # Not supported yet
                continue

            if modifiers & SymmapModifiers.SHIFT and not shifted:
                yield (shift_key, True)
                shifted = True
            elif not (modifiers & SymmapModifiers.SHIFT) and shifted:
                yield (shift_key, False)
                shifted = False

            yield (key, True)
            yield (key, False)
            break

    if shifted:
        yield (shift_key, False)
