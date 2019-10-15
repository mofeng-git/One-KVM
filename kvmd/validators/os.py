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


import os

from typing import List
from typing import Any

from . import raise_error
from . import check_not_none_string

from .basic import valid_number
from .basic import valid_string_list


# =====
def valid_abs_path(arg: Any, exists: bool=False, name: str="") -> str:
    if not name:
        name = ("existent absolute path" if exists else "absolute path")

    if len(str(arg).strip()) == 0:
        arg = None
    arg = check_not_none_string(arg, name)

    arg = os.path.abspath(arg)
    if exists and not os.access(arg, os.F_OK):
        raise_error(arg, name)
    return arg


def valid_abs_path_exists(arg: Any, name: str="") -> str:
    return valid_abs_path(arg, exists=True, name=name)


def valid_printable_filename(arg: Any, name: str="") -> str:
    if not name:
        name = "printable filename"

    if len(str(arg).strip()) == 0:
        arg = None
    arg = check_not_none_string(arg, name)

    if "/" in arg or "\0" in arg or arg in [".", ".."]:
        raise_error(arg, name)

    arg = "".join(
        (ch if ch.isprintable() else "_")
        for ch in arg[:255]
    )
    return arg


# =====
def valid_unix_mode(arg: Any) -> int:
    return int(valid_number(arg, min=0, name="UNIX mode"))


def valid_command(arg: Any) -> List[str]:
    cmd = valid_string_list(arg, delim=r"[,\t]+", name="command")
    if len(cmd) == 0:
        raise_error(arg, "command")
    cmd[0] = valid_abs_path_exists(cmd[0], name="command entry point")
    return cmd
