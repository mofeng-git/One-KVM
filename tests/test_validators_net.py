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
from kvmd.validators.net import valid_ip_or_host
from kvmd.validators.net import valid_ip
from kvmd.validators.net import valid_rfc_host
from kvmd.validators.net import valid_port


# =====
@pytest.mark.parametrize("arg", [
    "yandex.ru ",
    "foobar",
    "foo-bar.ru",
    "127.0.0.1",
    "8.8.8.8",
    "::",
    "::1",
    "2001:500:2f::f",
])
def test_ok__valid_ip_or_host(arg: Any) -> None:
    assert valid_ip_or_host(arg) == arg.strip()


@pytest.mark.parametrize("arg", [
    "foo_bar.ru",
    "1.1.1.",
    ":",
    "",
    None,
])
def test_fail__valid_ip_or_host(arg: Any) -> None:
    with pytest.raises(ValidatorError):
        print(valid_ip_or_host(arg))


# =====
@pytest.mark.parametrize("arg", [
    "127.0.0.1 ",
    "8.8.8.8",
    "::",
    "::1",
    "2001:500:2f::f",
])
def test_ok__valid_ip(arg: Any) -> None:
    assert valid_ip(arg) == arg.strip()


@pytest.mark.parametrize("arg", [
    "ya.ru",
    "1",
    "1.1.1",
    "1.1.1.",
    ":",
    "",
    None,
])
def test__fail_valid_ip(arg: Any) -> None:
    with pytest.raises(ValidatorError):
        print(valid_ip(arg))


# =====
@pytest.mark.parametrize("arg", [
    "yandex.ru ",
    "foobar",
    "foo-bar.ru",
    "z0r.de",
    "11.ru",
    "127.0.0.1",
])
def test_ok__valid_rfc_host(arg: Any) -> None:
    assert valid_rfc_host(arg) == arg.strip()


@pytest.mark.parametrize("arg", [
    "foobar.ru.",
    "foo_bar.ru",
    "",
    None,
])
def test_fail__valid_rfc_host(arg: Any) -> None:
    with pytest.raises(ValidatorError):
        print(valid_rfc_host(arg))


# =====
@pytest.mark.parametrize("arg", ["0 ", 0, "22", 443, 65535])
def test_ok__valid_port(arg: Any) -> None:
    value = valid_port(arg)
    assert type(value) == int  # pylint: disable=unidiomatic-typecheck
    assert value == int(str(arg).strip())


@pytest.mark.parametrize("arg", ["test", "", None, 1.1])
def test_fail__valid_port(arg: Any) -> None:
    with pytest.raises(ValidatorError):
        print(valid_port(arg))
