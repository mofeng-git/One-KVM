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

from kvmd.validators import ValidatorError
from kvmd.validators.kvm import valid_atx_button
from kvmd.validators.kvm import valid_kvm_target
from kvmd.validators.kvm import valid_log_seek
from kvmd.validators.kvm import valid_stream_quality
from kvmd.validators.kvm import valid_stream_fps


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
