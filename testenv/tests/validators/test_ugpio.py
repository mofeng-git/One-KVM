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


from typing import Callable
from typing import Any

import pytest

from kvmd.validators import ValidatorError
from kvmd.validators.ugpio import valid_ugpio_driver
from kvmd.validators.ugpio import valid_ugpio_channel
from kvmd.validators.ugpio import valid_ugpio_mode
from kvmd.validators.ugpio import valid_ugpio_view_table

from kvmd.plugins.ugpio import UserGpioModes


# =====
@pytest.mark.parametrize("validator", [valid_ugpio_driver, valid_ugpio_channel])
@pytest.mark.parametrize("arg", [
    "test-",
    "glados",
    "test",
    "_",
    "_foo_bar_",
    " aix",
    "a" * 255,
])
def test_ok__valid_ugpio_item(validator: Callable[[Any], str], arg: Any) -> None:
    assert validator(arg) == arg.strip()


@pytest.mark.parametrize("validator", [valid_ugpio_driver, valid_ugpio_channel])
@pytest.mark.parametrize("arg", [
    "тест",
    "-molestia",
    "te~st",
    "-",
    "-foo_bar",
    "foo bar",
    "a" * 256,
    "  ",
    "",
    None,
])
def test_fail__valid_ugpio_item(validator: Callable[[Any], str], arg: Any) -> None:
    with pytest.raises(ValidatorError):
        print(validator(arg))


# =====
@pytest.mark.parametrize("arg", ["foo", " bar", " baz "])
def test_ok__valid_ugpio_driver_variants(arg: Any) -> None:
    value = valid_ugpio_driver(arg, set(["foo", "bar", "baz"]))
    assert type(value) == str  # pylint: disable=unidiomatic-typecheck
    assert value == str(arg).strip()


@pytest.mark.parametrize("arg", ["BAR", " ", "", None])
def test_fail__valid_ugpio_driver_variants(arg: Any) -> None:
    with pytest.raises(ValidatorError):
        print(valid_ugpio_driver(arg, set(["foo", "bar", "baz"])))


# =====
@pytest.mark.parametrize("arg", ["Input ", " OUTPUT "])
def test_ok__valid_ugpio_mode(arg: Any) -> None:
    assert valid_ugpio_mode(arg, UserGpioModes.ALL) == arg.strip().lower()


@pytest.mark.parametrize("arg", ["test", "", None])
def test_fail__valid_ugpio_mode(arg: Any) -> None:
    with pytest.raises(ValidatorError):
        print(valid_ugpio_mode(arg, UserGpioModes.ALL))


# =====
@pytest.mark.parametrize("arg,retval", [
    ([],                     []),
    ({},                     []),
    ([[]],                   [[]]),
    ([{}],                   [[]]),
    ([[[]]],                 [["[]"]]),
    ("",                     []),
    ("ab",                   [["a"], ["b"]]),
    ([[1, 2], [None], "ab", {}, [3, 4]],   [["1", "2"], ["None"], ["a", "b"], [], ["3", "4"]]),
])
def test_ok__valid_ugpio_view_table(arg: Any, retval: Any) -> None:
    assert valid_ugpio_view_table(arg) == retval


@pytest.mark.parametrize("arg", [None, [None], 1])
def test_fail__valid_ugpio_view_table(arg: Any) -> None:
    with pytest.raises(ValidatorError):
        print(valid_ugpio_view_table(arg))
