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


from typing import Any

import pytest

from kvmd.keymap import KEYMAP

from kvmd.validators import ValidatorError
from kvmd.validators.kvm import valid_atx_power_action
from kvmd.validators.kvm import valid_atx_button
from kvmd.validators.kvm import valid_kvm_target
from kvmd.validators.kvm import valid_log_seek
from kvmd.validators.kvm import valid_stream_quality
from kvmd.validators.kvm import valid_stream_fps
from kvmd.validators.kvm import valid_hid_key
from kvmd.validators.kvm import valid_hid_mouse_move
from kvmd.validators.kvm import valid_hid_mouse_button
from kvmd.validators.kvm import valid_hid_mouse_wheel


# =====
@pytest.mark.parametrize("arg", ["ON ", "OFF ", "OFF_HARD ", "RESET_HARD "])
def test_ok__valid_atx_power_action(arg: Any) -> None:
    assert valid_atx_power_action(arg) == arg.strip().lower()


@pytest.mark.parametrize("arg", ["test", "", None])
def test_fail__valid_atx_power_action(arg: Any) -> None:
    with pytest.raises(ValidatorError):
        print(valid_atx_power_action(arg))


# =====
@pytest.mark.parametrize("arg", ["POWER ", "POWER_LONG ", "RESET "])
def test_ok__valid_atx_button(arg: Any) -> None:
    assert valid_atx_button(arg) == arg.strip().lower()


@pytest.mark.parametrize("arg", ["test", "", None])
def test_fail__valid_atx_button(arg: Any) -> None:
    with pytest.raises(ValidatorError):
        print(valid_atx_button(arg))


# =====
@pytest.mark.parametrize("arg", ["KVM ", "SERVER "])
def test_ok__valid_kvm_target(arg: Any) -> None:
    assert valid_kvm_target(arg) == arg.strip().lower()


@pytest.mark.parametrize("arg", ["test", "", None])
def test_fail__valid_kvm_target(arg: Any) -> None:
    with pytest.raises(ValidatorError):
        print(valid_kvm_target(arg))


# =====
@pytest.mark.parametrize("arg", ["0 ", 0, 1, 13])
def test_ok__valid_log_seek(arg: Any) -> None:
    value = valid_log_seek(arg)
    assert type(value) == int  # pylint: disable=unidiomatic-typecheck
    assert value == int(str(arg).strip())


@pytest.mark.parametrize("arg", ["test", "", None, -1, -13, 1.1])
def test_fail__valid_log_seek(arg: Any) -> None:
    with pytest.raises(ValidatorError):
        print(valid_log_seek(arg))


# =====
@pytest.mark.parametrize("arg", ["1 ", 20, 100])
def test_ok__valid_stream_quality(arg: Any) -> None:
    value = valid_stream_quality(arg)
    assert type(value) == int  # pylint: disable=unidiomatic-typecheck
    assert value == int(str(arg).strip())


@pytest.mark.parametrize("arg", ["test", "", None, 0, 101, 1.1])
def test_fail__valid_stream_quality(arg: Any) -> None:
    with pytest.raises(ValidatorError):
        print(valid_stream_quality(arg))


# =====
@pytest.mark.parametrize("arg", ["1 ", 30])
def test_ok__valid_stream_fps(arg: Any) -> None:
    value = valid_stream_fps(arg)
    assert type(value) == int  # pylint: disable=unidiomatic-typecheck
    assert value == int(str(arg).strip())


@pytest.mark.parametrize("arg", ["test", "", None, 31, 1.1])
def test_fail__valid_stream_fps(arg: Any) -> None:
    with pytest.raises(ValidatorError):
        print(valid_stream_fps(arg))


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
@pytest.mark.parametrize("arg", ["LEFT ", "RIGHT "])
def test_ok__valid_hid_mouse_button(arg: Any) -> None:
    assert valid_hid_mouse_button(arg) == arg.strip().lower()


@pytest.mark.parametrize("arg", ["test", "", None])
def test_fail__valid_hid_mouse_button(arg: Any) -> None:
    with pytest.raises(ValidatorError):
        print(valid_hid_mouse_button(arg))


# =====
@pytest.mark.parametrize("arg", [-100, "1 ", "-1", 1, -1, 0, "100 "])
def test_ok__valid_hid_mouse_wheel(arg: Any) -> None:
    assert valid_hid_mouse_wheel(arg) == int(str(arg).strip())


def test_ok__valid_hid_mouse_wheel__m200() -> None:
    assert valid_hid_mouse_wheel(-200) == -128


def test_ok__valid_hid_mouse_wheel__p200() -> None:
    assert valid_hid_mouse_wheel(200) == 127


@pytest.mark.parametrize("arg", ["test", "", None, 1.1])
def test_fail__valid_hid_mouse_wheel(arg: Any) -> None:
    with pytest.raises(ValidatorError):
        print(valid_hid_mouse_wheel(arg))
