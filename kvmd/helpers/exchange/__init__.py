# ========================================================================== #
#                                                                            #
#    KVMD - The main PiKVM daemon.                                           #
#                                                                            #
#    Copyright (C) 2018-2022  Maxim Devaev <mdevaev@gmail.com>               #
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


import sys
import os
import ctypes
import ctypes.util

from ctypes import c_int
from ctypes import c_uint
from ctypes import c_char_p


# =====
def main() -> None:
    if len(sys.argv) != 3:
        raise SystemExit(f"Usage: {sys.argv[0]} <file1> <file2>")

    path = ctypes.util.find_library("c")
    if not path:
        raise SystemExit("Where is libc?")
    assert path
    libc = ctypes.CDLL(path)
    libc.renameat2.restype = c_int
    libc.renameat2.argtypes = [c_int, c_char_p, c_int, c_char_p, c_uint]

    result = libc.renameat2(
        -100,  # AT_FDCWD
        os.fsencode(sys.argv[1]),
        -100,
        os.fsencode(sys.argv[2]),
        (1 << 1),  # RENAME_EXCHANGE
    )

    if result != 0:
        raise SystemExit(f"{sys.argv[0]}: {os.strerror(ctypes.get_errno())}")
