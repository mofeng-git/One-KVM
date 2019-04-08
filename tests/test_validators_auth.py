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
from kvmd.validators.auth import valid_user
from kvmd.validators.auth import valid_passwd
from kvmd.validators.auth import valid_auth_token
from kvmd.validators.auth import valid_auth_type


# =====
@pytest.mark.parametrize("arg", [
    "test-",
    "glados",
    "test",
    "_",
    "_foo_bar_",
    " aix",
])
def test_ok__valid_user(arg: Any) -> None:
    assert valid_user(arg) == arg.strip()


@pytest.mark.parametrize("arg", [
    "тест",
    "-molestia",
    "te~st",
    "-",
    "-foo_bar",
    "  ",
    "",
    None,
])
def test_fail__valid_user(arg: Any) -> None:
    with pytest.raises(ValidatorError):
        print(valid_user(arg))


# =====
@pytest.mark.parametrize("arg", [
    "glados",
    "test",
    "_",
    "_foo_bar_",
    " aix",
    "   ",
    "",
    " O(*#&@)FD*S)D(F   ",
])
def test_ok__valid_passwd(arg: Any) -> None:
    assert valid_passwd(arg) == arg


@pytest.mark.parametrize("arg", [
    "тест",
    "\n",
    " \n",
    "\n\n",
    "\r",
    None,
])
def test_fail__valid_passwd(arg: Any) -> None:
    with pytest.raises(ValidatorError):
        print(valid_passwd(arg))


# =====
@pytest.mark.parametrize("arg", [
    ("0" * 64) + " ",
    ("f" * 64) + " ",
])
def test_ok__valid_auth_token(arg: Any) -> None:
    assert valid_auth_token(arg) == arg.strip()


@pytest.mark.parametrize("arg", [
    ("F" * 64),
    "0" * 63,
    "0" * 65,
    "",
    None,
])
def test_fail__valid_auth_token(arg: Any) -> None:
    with pytest.raises(ValidatorError):
        print(valid_auth_token(arg))


@pytest.mark.parametrize("arg", ["HTPASSWD ", "htpasswd"])
def test_ok__valid_auth_type(arg: Any) -> None:
    assert valid_auth_type(arg) == arg.strip().lower()


@pytest.mark.parametrize("arg", ["test", "", None])
def test_fail__valid_auth_type(arg: Any) -> None:
    with pytest.raises(ValidatorError):
        print(valid_auth_type(arg))
