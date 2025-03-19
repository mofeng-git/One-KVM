# ========================================================================== #
#                                                                            #
#    KVMD - The main PiKVM daemon.                                           #
#                                                                            #
#    Copyright (C) 2018-2024  Maxim Devaev <mdevaev@gmail.com>               #
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
from kvmd.validators.switch import valid_switch_port_name
from kvmd.validators.switch import valid_switch_edid_id
from kvmd.validators.switch import valid_switch_edid_data
from kvmd.validators.switch import valid_switch_color
from kvmd.validators.switch import valid_switch_atx_click_delay


# =====
@pytest.mark.parametrize("arg, retval", [
    ("\tMac OS Host  #1/..", "Mac OS Host #1/.."),
    ("\t",                   ""),
    ("",                     ""),
])
def test_ok__valid_msd_image_name(arg: Any, retval: str) -> None:
    assert valid_switch_port_name(arg) == retval


@pytest.mark.parametrize("arg", [None])
def test_fail__valid_msd_image_name(arg: Any) -> None:
    with pytest.raises(ValidatorError):
        valid_switch_port_name(arg)


# =====
@pytest.mark.parametrize("arg", [
    "550e8400-e29b-41d4-a716-446655440000",
    " 00000000-0000-0000-C000-000000000046 ",
    " 00000000-0000-0000-0000-000000000000 ",
])
def test_ok__valid_switch_edid_id__no_default(arg: Any) -> None:
    assert valid_switch_edid_id(arg, allow_default=False) == arg.strip().lower()  # type: ignore


@pytest.mark.parametrize("arg", [
    "550e8400-e29b-41d4-a716-44665544",
    "ffffuuuu-0000-0000-C000-000000000046",
    "default",
    "",
    None,
])
def test_fail__valid_switch_edid_id__no_default(arg: Any) -> None:
    with pytest.raises(ValidatorError):
        valid_switch_edid_id(arg, allow_default=False)


# =====
@pytest.mark.parametrize("arg", [
    "550e8400-e29b-41d4-a716-446655440000",
    " 00000000-0000-0000-C000-000000000046 ",
    " 00000000-0000-0000-0000-000000000000 ",
    " Default",
])
def test_ok__valid_switch_edid_id__allowed_default(arg: Any) -> None:
    assert valid_switch_edid_id(arg, allow_default=True) == arg.strip().lower()  # type: ignore


@pytest.mark.parametrize("arg", [
    "550e8400-e29b-41d4-a716-44665544",
    "ffffuuuu-0000-0000-C000-000000000046",
    "",
    None,
])
def test_fail__valid_switch_edid_id__allowed_default(arg: Any) -> None:
    with pytest.raises(ValidatorError):
        valid_switch_edid_id(arg, allow_default=True)


# =====
@pytest.mark.parametrize("arg", [
    "f" * 256,
    "0" * 256,
    "1a" * 128,
    "f" * 512,
    "0" * 512,
    "1a" * 256,
])
def test_ok__valid_switch_edid_data(arg: Any) -> None:
    assert valid_switch_edid_data(arg) == arg.upper()  # type: ignore


@pytest.mark.parametrize("arg", [
    "f" * 511,
    "0" * 511,
    "1a" * 255,
    "F" * 513,
    "0" * 513,
    "1A" * 257,
    "",
    None,
])
def test_fail__valid_switch_edid_data(arg: Any) -> None:
    with pytest.raises(ValidatorError):
        valid_switch_edid_data(arg)


# =====
@pytest.mark.parametrize("arg, retval", [
    ("000000:00:0000",   "000000:00:0000"),
    (" 0f0f0f:0f:0f0f ", "0F0F0F:0F:0F0F"),
])
def test_ok__valid_switch_color__no_default(arg: Any, retval: str) -> None:
    assert valid_switch_color(arg, allow_default=False) == retval


@pytest.mark.parametrize("arg", [
    "550e8400-e29b-41d4-a716-44665544",
    "ffffuuuu-0000-0000-C000-000000000046",
    "000000:00:000000000:00:000G",
    "000000:00:000",
    "000000:00:000G",
    "default",
    " Default",
    "",
    None,
])
def test_fail__valid_switch_color__no_default(arg: Any) -> None:
    with pytest.raises(ValidatorError):
        valid_switch_color(arg, allow_default=False)


# =====
@pytest.mark.parametrize("arg, retval", [
    ("000000:00:0000",   "000000:00:0000"),
    (" 0f0f0f:0f:0f0f ", "0F0F0F:0F:0F0F"),
    (" Default",         "default"),
])
def test_ok__valid_switch_color__allow_default(arg: Any, retval: str) -> None:
    assert valid_switch_color(arg, allow_default=True) == retval


@pytest.mark.parametrize("arg", [
    "550e8400-e29b-41d4-a716-44665544",
    "ffffuuuu-0000-0000-C000-000000000046",
    "000000:00:000000000:00:000G",
    "000000:00:000",
    "000000:00:000G",
    "",
    None,
])
def test_fail__valid_switch_color__allow_default(arg: Any) -> None:
    with pytest.raises(ValidatorError):
        valid_switch_color(arg, allow_default=True)


# =====
@pytest.mark.parametrize("arg", [0, 1, 5, "5 ", "5.0 ", " 10"])
def test_ok__valid_switch_atx_click_delay(arg: Any) -> None:
    value = valid_switch_atx_click_delay(arg)
    assert type(value) is float  # pylint: disable=unidiomatic-typecheck
    assert value == float(str(arg).strip())


@pytest.mark.parametrize("arg", ["test", "", None, -6, "-6", "10.1"])
def test_fail__valid_switch_atx_click_delay(arg: Any) -> None:
    with pytest.raises(ValidatorError):
        print(valid_switch_atx_click_delay(arg))
