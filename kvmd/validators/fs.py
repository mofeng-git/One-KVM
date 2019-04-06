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

from typing import Any

from . import raise_error
from . import check_not_none_string

from .basic import valid_number


# =====
def valid_abs_path(arg: Any, exists: bool=False) -> str:
    name = ("existent absolute path" if exists else "absolute path")

    if len(str(arg).strip()) == 0:
        arg = None
    arg = check_not_none_string(arg, name)

    arg = os.path.abspath(arg)
    if exists and not os.access(arg, os.F_OK):
        raise_error(arg, name)
    return arg


def valid_abs_path_exists(arg: Any) -> str:
    return valid_abs_path(arg, exists=True)


def valid_unix_mode(arg: Any) -> int:
    return int(valid_number(arg, min=0, name="UNIX mode"))
