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


from typing import Tuple
from typing import Dict
from typing import Generator

from .keysym import SymmapWebKey


# =====
def text_to_web_keys(
    text: str,
    symmap: Dict[int, SymmapWebKey],
    shift_key: str="ShiftLeft",
) -> Generator[Tuple[str, bool], None, None]:

    assert shift_key in ["ShiftLeft", "ShiftRight"]

    shifted = False
    for ch in text:
        try:
            code = ord(ch)
            if not (0x20 <= code <= 0x7E):
                # https://stackoverflow.com/questions/12343987/convert-ascii-character-to-x11-keycode
                # https://www.ascii-code.com
                continue
            key = symmap[code]
        except Exception:
            continue
        if key.altgr or key.ctrl:
            continue  # Not supported yet

        if key.shift and not shifted:
            yield (shift_key, True)
            shifted = True
        elif not key.shift and shifted:
            yield (shift_key, False)
            shifted = False

        yield (key.name, True)
        yield (key.name, False)

    if shifted:
        yield (shift_key, False)
