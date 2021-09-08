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

from kvmd.keyboard.mappings import KEYMAP

from kvmd.validators import ValidatorError
from kvmd.validators.hid import valid_hid_key
from kvmd.validators.hid import valid_hid_mouse_move
from kvmd.validators.hid import valid_hid_mouse_button
from kvmd.validators.hid import valid_hid_mouse_delta


# =====
def test_ok__valid_hid_key() -> None:
    for key in KEYMAP:
        print(valid_hid_key(key))
        print(valid_hid_key(key + " "))


@pytest.mark.parametrize("arg", ["test", "", None, "keya"])
def test_fail__valid_hid_key(arg: Any) -> None:
    with pytest.raises(ValidatorError):
        print(valid_hid_key(arg))


# =====
@pytest.mark.parametrize("arg", [-20000, "1 ", "-1", 1, -1, 0, "20000 "])
def test_ok__valid_hid_mouse_move(arg: Any) -> None:
    assert valid_hid_mouse_move(arg) == int(str(arg).strip())


def test_ok__valid_hid_mouse_move__m50000() -> None:
    assert valid_hid_mouse_move(-50000) == -32768


def test_ok__valid_hid_mouse_move__p50000() -> None:
    assert valid_hid_mouse_move(50000) == 32767


@pytest.mark.parametrize("arg", ["test", "", None, 1.1])
def test_fail__valid_hid_mouse_move(arg: Any) -> None:
    with pytest.raises(ValidatorError):
        print(valid_hid_mouse_move(arg))


# =====
@pytest.mark.parametrize("arg", ["LEFT ", "RIGHT ", "Up ", " Down", " MiDdLe "])
def test_ok__valid_hid_mouse_button(arg: Any) -> None:
    assert valid_hid_mouse_button(arg) == arg.strip().lower()


@pytest.mark.parametrize("arg", ["test", "", None])
def test_fail__valid_hid_mouse_button(arg: Any) -> None:
    with pytest.raises(ValidatorError):
        print(valid_hid_mouse_button(arg))


# =====
@pytest.mark.parametrize("arg", [-100, "1 ", "-1", 1, -1, 0, "100 "])
def test_ok__valid_hid_mouse_delta(arg: Any) -> None:
    assert valid_hid_mouse_delta(arg) == int(str(arg).strip())


def test_ok__valid_hid_mouse_delta__m200() -> None:
    assert valid_hid_mouse_delta(-200) == -127


def test_ok__valid_hid_mouse_delta__p200() -> None:
    assert valid_hid_mouse_delta(200) == 127


@pytest.mark.parametrize("arg", ["test", "", None, 1.1])
def test_fail__valid_hid_mouse_delta(arg: Any) -> None:
    with pytest.raises(ValidatorError):
        print(valid_hid_mouse_delta(arg))
