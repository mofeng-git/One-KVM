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
from kvmd.validators.kvm import valid_atx_power_action
from kvmd.validators.kvm import valid_atx_button
from kvmd.validators.kvm import valid_info_fields
from kvmd.validators.kvm import valid_log_seek
from kvmd.validators.kvm import valid_stream_quality
from kvmd.validators.kvm import valid_stream_fps
from kvmd.validators.kvm import valid_stream_resolution
from kvmd.validators.kvm import valid_stream_h264_bitrate
from kvmd.validators.kvm import valid_stream_h264_gop


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
@pytest.mark.parametrize("arg", [" foo ", "bar", "foo, ,bar,", " ", " , ", ""])
def test_ok__valid_info_fields(arg: Any) -> None:
    value = valid_info_fields(arg, set(["foo", "bar"]))
    assert type(value) == set  # pylint: disable=unidiomatic-typecheck
    assert value == set(filter(None, map(str.strip, str(arg).split(","))))


@pytest.mark.parametrize("arg", ["xxx", "yyy", "foo,xxx", None])
def test_fail__valid_info_fields(arg: Any) -> None:
    with pytest.raises(ValidatorError):
        print(valid_info_fields(arg, set(["foo", "bar"])))


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
@pytest.mark.parametrize("arg", ["1 ", 120])
def test_ok__valid_stream_fps(arg: Any) -> None:
    value = valid_stream_fps(arg)
    assert type(value) == int  # pylint: disable=unidiomatic-typecheck
    assert value == int(str(arg).strip())


@pytest.mark.parametrize("arg", ["test", "", None, 121, 1.1])
def test_fail__valid_stream_fps(arg: Any) -> None:
    with pytest.raises(ValidatorError):
        print(valid_stream_fps(arg))


# =====
@pytest.mark.parametrize("arg", ["1280x720 ", "1x1"])
def test_ok__valid_stream_resolution(arg: Any) -> None:
    value = valid_stream_resolution(arg)
    assert type(value) == str  # pylint: disable=unidiomatic-typecheck
    assert value == str(arg).strip()


@pytest.mark.parametrize("arg", ["x", None, "0x0", "0x1", "1x0", "1280", "1280x", "1280x720x"])
def test_fail__valid_stream_resolution(arg: Any) -> None:
    with pytest.raises(ValidatorError):
        print(valid_stream_resolution(arg))


# =====
@pytest.mark.parametrize("arg", ["100", " 16000 ", 5000])
def test_ok__valid_stream_h264_bitrate(arg: Any) -> None:
    value = valid_stream_h264_bitrate(arg)
    assert type(value) == int  # pylint: disable=unidiomatic-typecheck
    assert value == int(str(arg).strip())


@pytest.mark.parametrize("arg", ["0", "-1", "100.0", 5000.1, None, ""])
def test_fail__valid_stream_h264_bitrate(arg: Any) -> None:
    with pytest.raises(ValidatorError):
        print(valid_stream_h264_bitrate(arg))


# =====
@pytest.mark.parametrize("arg", ["1 ", 0, 60])
def test_ok__valid_stream_h264_gop(arg: Any) -> None:
    value = valid_stream_h264_gop(arg)
    assert type(value) == int  # pylint: disable=unidiomatic-typecheck
    assert value == int(str(arg).strip())


@pytest.mark.parametrize("arg", ["test", "", None, 61, 1.1])
def test_fail__valid_stream_h264_gop(arg: Any) -> None:
    with pytest.raises(ValidatorError):
        print(valid_stream_h264_gop(arg))
