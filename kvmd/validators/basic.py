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


from typing import Type
from typing import Union
from typing import Any

from . import ValidatorError
from . import raise_error
from . import check_not_none_string
from . import check_in_list


# =====
def valid_bool(arg: Any) -> bool:
    true_args = ["1", "true", "yes"]
    false_args = ["0", "false", "no"]

    name = "bool (%r or %r)" % (true_args, false_args)

    arg = check_not_none_string(arg, name).lower()
    arg = check_in_list(arg, name, true_args + false_args)
    return (arg in true_args)


def valid_number(
    arg: Any,
    min: Union[int, float, None]=None,  # pylint: disable=redefined-builtin
    max: Union[int, float, None]=None,  # pylint: disable=redefined-builtin
    type: Union[Type[int], Type[float]]=int,  # pylint: disable=redefined-builtin
    name: str="",
) -> Union[int, float]:

    name = (name or type.__name__)

    arg = check_not_none_string(arg, name)
    try:
        arg = type(arg)
    except Exception:
        raise_error(arg, name)

    if min is not None and arg < min:
        raise ValidatorError("The argument '%s' must be %s and greater or equial than %s" % (arg, name, min))
    if max is not None and arg > max:
        raise ValidatorError("The argument '%s' must be %s and lesser or equal then %s" % (arg, name, max))
    return arg


def valid_int_f1(arg: Any) -> int:
    return int(valid_number(arg, min=1))


def valid_float_f01(arg: Any) -> float:
    return float(valid_number(arg, min=0.1, type=float))
