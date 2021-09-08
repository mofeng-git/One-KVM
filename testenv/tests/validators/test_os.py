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


import os

from typing import List
from typing import Any

import pytest

from kvmd.validators import ValidatorError
from kvmd.validators.os import valid_abs_path
from kvmd.validators.os import valid_printable_filename
from kvmd.validators.os import valid_unix_mode
from kvmd.validators.os import valid_command


# =====
@pytest.mark.parametrize("arg, retval", [
    ("/..",          "/"),
    ("/root/..",     "/"),
    ("/root",        "/root"),
    ("/f/o/o/b/a/r", "/f/o/o/b/a/r"),
    ("~",            os.path.abspath(".") + "/~"),
    ("/foo~",        "/foo~"),
    ("/foo/~",        "/foo/~"),
    (".",            os.path.abspath(".")),
])
def test_ok__valid_abs_path(arg: Any, retval: str) -> None:
    assert valid_abs_path(arg) == retval


@pytest.mark.parametrize("arg", ["", " ", None])
def test_fail__valid_abs_path(arg: Any) -> None:
    with pytest.raises(ValidatorError):
        print(valid_abs_path(arg))


# =====
@pytest.mark.parametrize("arg, retval", [
    ("/..",          "/"),
    ("/root/..",     "/"),
    ("/root",        "/root"),
    (".",            os.path.abspath(".")),
])
def test_ok__valid_abs_path__dir(arg: Any, retval: str) -> None:
    assert valid_abs_path(arg, type="dir") == retval


@pytest.mark.parametrize("arg", [
    "/etc/passwd",
    "/etc/passwd/",
    "~",
    "/foo~",
    "/foo/~",
    "",
    None,
])
def test_fail__valid_abs_path__dir(arg: Any) -> None:
    with pytest.raises(ValidatorError):
        print(valid_abs_path(arg, type="dir"))


# =====
@pytest.mark.parametrize("arg, retval", [
    ("archlinux-2018.07.01-i686.iso",   "archlinux-2018.07.01-i686.iso"),
    ("archlinux-2018.07.01-x86_64.iso", "archlinux-2018.07.01-x86_64.iso"),
    ("dsl-4.11.rc1.iso",                "dsl-4.11.rc1.iso"),
    ("systemrescuecd-x86-5.3.1.iso",    "systemrescuecd-x86-5.3.1.iso"),
    ("ubuntu-16.04.5-desktop-i386.iso", "ubuntu-16.04.5-desktop-i386.iso"),
    (" тест(){}[ \t].iso\t", "тест(){}[ _].iso"),
    ("\n" + "x" * 1000,          "x" * 255),
    ("test",       "test"),
    ("test test [test] #test$", "test test [test] #test$"),
    (".test",      ".test"),
    ("..test",     "..test"),
    ("..тест..",   "..тест.."),
    ("..те\\ст..", "..те\\ст.."),
    (".....",      "....."),
    (".....txt",   ".....txt"),
    (" .. .",      ".. ."),
    ("..\n.",      ".._."),
])
def test_ok__valid_printable_filename(arg: Any, retval: str) -> None:
    assert valid_printable_filename(arg) == retval


@pytest.mark.parametrize("arg", [
    ".",
    "..",
    " ..",
    "test/",
    "/test",
    "../test",
    "./.",
    "../.",
    "./..",
    "../..",
    "/ ..",
    ".. /",
    "/.. /",
    "",
    " ",
    None,
])
def test_fail__valid_printable_filename(arg: Any) -> None:
    with pytest.raises(ValidatorError):
        valid_printable_filename(arg)


# =====
@pytest.mark.parametrize("arg", [0, 5, "1000"])
def test_ok__valid_unix_mode(arg: Any) -> None:
    value = valid_unix_mode(arg)
    assert type(value) == int  # pylint: disable=unidiomatic-typecheck
    assert value == int(str(value).strip())


@pytest.mark.parametrize("arg", ["test", "", None, -6, "-6", "5.0"])
def test_fail__valid_unix_mode(arg: Any) -> None:
    with pytest.raises(ValidatorError):
        print(valid_unix_mode(arg))


# =====
@pytest.mark.parametrize("arg, retval", [
    (["/bin/true"],          ["/bin/true"]),
    (["/bin/true", 1, 2, 3], ["/bin/true", "1", "2", "3"]),
    ("/bin/true, 1, 2, 3,",  ["/bin/true", "1", "2", "3"]),
    ("/bin/true",            ["/bin/true"]),
])
def test_ok__valid_command(arg: Any, retval: List[str]) -> None:
    assert valid_command(arg) == retval


@pytest.mark.parametrize("arg", [
    ["/bin/blahblahblah"],
    ["/bin/blahblahblah", 1, 2, 3],
    [" "],
    [],
    "/bin/blahblahblah, 1, 2, 3,",
    "/bin/blahblahblah",
    " ",
])
def test_fail__valid_command(arg: Any) -> None:
    with pytest.raises(ValidatorError):
        print(valid_command(arg))
