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


from typing import List
from typing import Any

import pytest

from kvmd.validators import ValidatorError
from kvmd.validators.basic import valid_bool
from kvmd.validators.basic import valid_number
from kvmd.validators.basic import valid_int_f0
from kvmd.validators.basic import valid_int_f1
from kvmd.validators.basic import valid_float_f0
from kvmd.validators.basic import valid_float_f01
from kvmd.validators.basic import valid_string_list


# =====
@pytest.mark.parametrize("arg, retval", [
    ("1",     True),
    ("true",  True),
    ("TRUE",  True),
    ("yes ",  True),
    (1,       True),
    (True,    True),
    ("0",     False),
    ("false", False),
    ("FALSE", False),
    ("no ",   False),
    (0,       False),
    (False,   False),
])
def test_ok__valid_bool(arg: Any, retval: bool) -> None:
    assert valid_bool(arg) == retval


@pytest.mark.parametrize("arg", ["test", "", None, -1, "x"])
def test_fail__valid_bool(arg: Any) -> None:
    with pytest.raises(ValidatorError):
        print(valid_bool(arg))


# =====
@pytest.mark.parametrize("arg", ["1 ", "-1", 1, -1, 0, 100500])
def test_ok__valid_number(arg: Any) -> None:
    assert valid_number(arg) == int(str(arg).strip())


@pytest.mark.parametrize("arg", ["test", "", None, "1x", 100500.0])
def test_fail__valid_number(arg: Any) -> None:
    with pytest.raises(ValidatorError):
        print(valid_number(arg))


@pytest.mark.parametrize("arg", [-5, 0, 5, "-5 ", "0 ", "5 "])
def test_ok__valid_number__min_max(arg: Any) -> None:
    assert valid_number(arg, -5, 5) == int(str(arg).strip())


@pytest.mark.parametrize("arg", ["test", "", None, -6, 6, "-6 ", "6 "])
def test_fail__valid_number__min_max(arg: Any) -> None:  # pylint: disable=invalid-name
    with pytest.raises(ValidatorError):
        print(valid_number(arg, -5, 5))


# =====
@pytest.mark.parametrize("arg", [0, 1, 5, "5 "])
def test_ok__valid_int_f0(arg: Any) -> None:
    value = valid_int_f0(arg)
    assert type(value) == int  # pylint: disable=unidiomatic-typecheck
    assert value == int(str(arg).strip())


@pytest.mark.parametrize("arg", ["test", "", None, -6, "-6 ", "5.0"])
def test_fail__valid_int_f0(arg: Any) -> None:
    with pytest.raises(ValidatorError):
        print(valid_int_f0(arg))


# =====
@pytest.mark.parametrize("arg", [1, 5, "5 "])
def test_ok__valid_int_f1(arg: Any) -> None:
    value = valid_int_f1(arg)
    assert type(value) == int  # pylint: disable=unidiomatic-typecheck
    assert value == int(str(arg).strip())


@pytest.mark.parametrize("arg", ["test", "", None, -6, "-6 ", 0, "0 ", "5.0"])
def test_fail__valid_int_f1(arg: Any) -> None:
    with pytest.raises(ValidatorError):
        print(valid_int_f1(arg))


# =====
@pytest.mark.parametrize("arg", [0, 1, 5, "5 ", "5.0 "])
def test_ok__valid_float_f0(arg: Any) -> None:
    value = valid_float_f0(arg)
    assert type(value) == float  # pylint: disable=unidiomatic-typecheck
    assert value == float(str(arg).strip())


@pytest.mark.parametrize("arg", ["test", "", None, -6, "-6"])
def test_fail__valid_float_f0(arg: Any) -> None:
    with pytest.raises(ValidatorError):
        print(valid_float_f0(arg))


# =====
@pytest.mark.parametrize("arg", [0.1, 1, 5, "5 ", "5.0 "])
def test_ok__valid_float_f01(arg: Any) -> None:
    value = valid_float_f01(arg)
    assert type(value) == float  # pylint: disable=unidiomatic-typecheck
    assert value == float(str(arg).strip())


@pytest.mark.parametrize("arg", ["test", "", None, 0.0, "0.0", -6, "-6", 0, "0"])
def test_fail__valid_float_f01(arg: Any) -> None:
    with pytest.raises(ValidatorError):
        print(valid_float_f01(arg))


# =====
@pytest.mark.parametrize("arg, retval", [
    ("a, b, c",       ["a", "b", "c"]),
    ("a, b,, c",      ["a", "b", "c"]),
    ("a b c",         ["a", "b", "c"]),
    (["a", "b", "c"], ["a", "b", "c"]),
    ("",              []),
    (" ",             []),
    (", ",            []),
    (", a, ",         ["a"]),
    ([],              []),
])
def test_ok__valid_string_list(arg: Any, retval: List) -> None:
    assert valid_string_list(arg) == retval


@pytest.mark.parametrize("arg, retval", [
    ("1, 2, 3", [1, 2, 3]),
    ("1 2 3",   [1, 2, 3]),
    ([1, 2, 3], [1, 2, 3]),
    ("",        []),
    (" ",       []),
    (", ",      []),
    (", 1, ",   [1]),
    ([],        []),
])
def test_ok__valid_string_list__subval(arg: Any, retval: List) -> None:  # pylint: disable=invalid-name
    assert valid_string_list(arg, subval=int) == retval


@pytest.mark.parametrize("arg", [None, [None]])
def test_fail__valid_string_list(arg: Any) -> None:  # pylint: disable=invalid-name
    with pytest.raises(ValidatorError):
        print(valid_string_list(arg))
