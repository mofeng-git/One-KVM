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


from typing import Any

import pytest

from kvmd.validators import ValidatorError
from kvmd.validators.hw import valid_tty_speed
from kvmd.validators.hw import valid_gpio_pin
from kvmd.validators.hw import valid_gpio_pin_optional
from kvmd.validators.hw import valid_otg_gadget
from kvmd.validators.hw import valid_otg_id
from kvmd.validators.hw import valid_otg_ethernet


# =====
@pytest.mark.parametrize("arg", ["1200 ", 1200, 2400, 4800, 9600, 19200, 38400, 57600, 115200])
def test_ok__valid_tty_speed(arg: Any) -> None:
    value = valid_tty_speed(arg)
    assert type(value) == int  # pylint: disable=unidiomatic-typecheck
    assert value == int(str(arg).strip())


@pytest.mark.parametrize("arg", ["test", "", None, 0, 1200.1])
def test_fail__valid_tty_speed(arg: Any) -> None:
    with pytest.raises(ValidatorError):
        print(valid_tty_speed(arg))


# =====
@pytest.mark.parametrize("arg", ["0 ", 0, 1, 13])
def test_ok__valid_gpio_pin(arg: Any) -> None:
    value = valid_gpio_pin(arg)
    assert type(value) == int  # pylint: disable=unidiomatic-typecheck
    assert value == int(str(arg).strip())


@pytest.mark.parametrize("arg", ["test", "", None, -1, -13, 1.1])
def test_fail__valid_gpio_pin(arg: Any) -> None:
    with pytest.raises(ValidatorError):
        print(valid_gpio_pin(arg))


# =====
@pytest.mark.parametrize("arg", ["0 ", -1, 0, 1, 13])
def test_ok__valid_gpio_pin_optional(arg: Any) -> None:
    value = valid_gpio_pin_optional(arg)
    assert type(value) == int  # pylint: disable=unidiomatic-typecheck
    assert value == int(str(arg).strip())


@pytest.mark.parametrize("arg", ["test", "", None, -2, -13, 1.1])
def test_fail__valid_gpio_pin_optional(arg: Any) -> None:
    with pytest.raises(ValidatorError):
        print(valid_gpio_pin_optional(arg))


# =====
@pytest.mark.parametrize("arg", [
    "test-",
    "glados",
    "test",
    "_",
    "_foo_bar_",
    " aix",
    "a" * 255,
])
def test_ok__valid_otg_gadget(arg: Any) -> None:
    assert valid_otg_gadget(arg) == arg.strip()


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
def test_fail__valid_otg_gadget(arg: Any) -> None:
    with pytest.raises(ValidatorError):
        print(valid_otg_gadget(arg))


# =====
@pytest.mark.parametrize("arg", ["0 ", 0, 1, 13, 65535])
def test_ok__valid_otg_id(arg: Any) -> None:
    value = valid_otg_id(arg)
    assert type(value) == int  # pylint: disable=unidiomatic-typecheck
    assert value == int(str(arg).strip())


@pytest.mark.parametrize("arg", ["test", "", None, -1, -13, 1.1, 65534.5, 65536])
def test_fail__valid_otg_id(arg: Any) -> None:
    with pytest.raises(ValidatorError):
        print(valid_otg_id(arg))


# =====
@pytest.mark.parametrize("arg", ["ECM ", "EeM ", "ncm ", " Rndis"])
def test_ok__valid_otg_ethernet(arg: Any) -> None:
    assert valid_otg_ethernet(arg) == arg.strip().lower()


@pytest.mark.parametrize("arg", ["test", "", None])
def test_fail__valid_otg_ethernet(arg: Any) -> None:
    with pytest.raises(ValidatorError):
        print(valid_otg_ethernet(arg))
