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


import re

from typing import Type
from typing import Callable
from typing import Any

from . import ValidatorError
from . import raise_error
from . import check_not_none_string
from . import check_in_list


# =====
def valid_stripped_string(arg: Any, name: str="") -> str:
    if not name:
        name = "stripped string"
    return check_not_none_string(arg, name)


def valid_stripped_string_not_empty(arg: Any, name: str="") -> str:
    if not name:
        name = "not empty stripped string"
    if len(str(arg).strip()) == 0:
        arg = None
    return valid_stripped_string(arg, name)


def valid_bool(arg: Any) -> bool:
    true_args = ["1", "true", "yes"]
    false_args = ["0", "false", "no"]

    name = f"bool ({true_args!r} or {false_args!r})"

    arg = valid_stripped_string_not_empty(arg, name).lower()
    arg = check_in_list(arg, name, true_args + false_args)
    return (arg in true_args)


def valid_number(
    arg: Any,
    min: (int | float | None)=None,  # pylint: disable=redefined-builtin
    max: (int | float | None)=None,  # pylint: disable=redefined-builtin
    type: (Type[int] | Type[float])=int,  # pylint: disable=redefined-builtin
    name: str="",
) -> (int | float):

    name = (name or type.__name__)

    arg = valid_stripped_string_not_empty(arg, name)
    try:
        arg = type(arg)
    except Exception:
        raise_error(arg, name)

    if min is not None and arg < min:
        raise ValidatorError(f"The argument '{arg}' must be {name} and greater or equial than {min}")
    if max is not None and arg > max:
        raise ValidatorError(f"The argument '{arg}' must be {name} and lesser or equal then {max}")
    return arg


def valid_int_f0(arg: Any) -> int:
    return int(valid_number(arg, min=0))


def valid_int_f1(arg: Any) -> int:
    return int(valid_number(arg, min=1))


def valid_float_f0(arg: Any) -> float:
    return float(valid_number(arg, min=0, type=float))


def valid_float_f01(arg: Any) -> float:
    return float(valid_number(arg, min=0.1, type=float))


def valid_string_list(
    arg: Any,
    delim: str=r"[,\t ]+",
    subval: (Callable[[Any], Any] | None)=None,
    name: str="",
) -> list[str]:

    if not name:
        name = "string list"

    if subval is None:
        subval = (lambda item: check_not_none_string(item, name + " item"))

    if not isinstance(arg, (list, tuple)):
        arg = check_not_none_string(arg, name)
        arg = list(filter(None, re.split(delim, arg)))

    try:
        arg = list(map(subval, arg))
    except Exception:
        raise ValidatorError(f"Failed sub-validator on one of the item of {arg!r}")
    return arg
