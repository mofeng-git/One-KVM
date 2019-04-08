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


import os
import hashlib
import tempfile
import builtins
import getpass

from typing import List
from typing import Generator
from typing import Any

import passlib.apache

import pytest

from kvmd.apps.htpasswd import main


# =====
def _make_passwd(user: str) -> str:
    return hashlib.md5(user.encode()).hexdigest()


@pytest.fixture(name="htpasswd", params=[[], ["admin"], ["admin", "user"]])
def _htpasswd_fixture(request) -> Generator[passlib.apache.HtpasswdFile, None, None]:  # type: ignore
    (fd, path) = tempfile.mkstemp()
    os.close(fd)
    htpasswd = passlib.apache.HtpasswdFile(path)
    for user in request.param:
        htpasswd.set_password(user, _make_passwd(user))
    htpasswd.save()
    yield htpasswd
    os.remove(path)


def _run_htpasswd(htpasswd: passlib.apache.HtpasswdFile, cmd: List[str]) -> None:
    main([
        "kvmd-htpasswd",
        *cmd,
        "--set-options",
        "kvmd/auth/htpasswd/file=" + htpasswd.path,
    ])


# =====
def test_ok__list(htpasswd: passlib.apache.HtpasswdFile, capsys) -> None:  # type: ignore
    _run_htpasswd(htpasswd, ["list"])
    (out, err) = capsys.readouterr()
    assert len(err) == 0
    assert sorted(filter(None, out.split("\n"))) == sorted(htpasswd.users()) == sorted(set(htpasswd.users()))


# =====
def test_ok__set_change_stdin(htpasswd: passlib.apache.HtpasswdFile, mocker) -> None:  # type: ignore
    old_users = set(htpasswd.users())
    if old_users:
        assert htpasswd.check_password("admin", _make_passwd("admin"))

        mocker.patch.object(builtins, "input", (lambda: " test "))
        _run_htpasswd(htpasswd, ["set", "admin", "--read-stdin"])

        htpasswd.load(force=True)
        assert htpasswd.check_password("admin", " test ")
        assert old_users == set(htpasswd.users())


def test_ok__set_add_stdin(htpasswd: passlib.apache.HtpasswdFile, mocker) -> None:  # type: ignore
    old_users = set(htpasswd.users())
    if old_users:
        mocker.patch.object(builtins, "input", (lambda: " test "))
        _run_htpasswd(htpasswd, ["set", "new", "--read-stdin"])

        htpasswd.load(force=True)
        assert htpasswd.check_password("new", " test ")
        assert old_users.union(["new"]) == set(htpasswd.users())


# =====
def test_ok__set_change_getpass(htpasswd: passlib.apache.HtpasswdFile, mocker) -> None:  # type: ignore
    old_users = set(htpasswd.users())
    if old_users:
        assert htpasswd.check_password("admin", _make_passwd("admin"))

        mocker.patch.object(getpass, "getpass", (lambda *_, **__: " test "))
        _run_htpasswd(htpasswd, ["set", "admin"])

        htpasswd.load(force=True)
        assert htpasswd.check_password("admin", " test ")
        assert old_users == set(htpasswd.users())


def test_fail__set_change_getpass(htpasswd: passlib.apache.HtpasswdFile, mocker) -> None:  # type: ignore
    old_users = set(htpasswd.users())
    if old_users:
        assert htpasswd.check_password("admin", _make_passwd("admin"))

        count = 0

        def fake_getpass(*_: Any, **__: Any) -> str:
            nonlocal count
            assert count <= 1
            if count == 0:
                passwd = " test "
            else:
                passwd = "test "
            count += 1
            return passwd

        mocker.patch.object(getpass, "getpass", fake_getpass)
        with pytest.raises(SystemExit, match="Sorry, passwords do not match"):
            _run_htpasswd(htpasswd, ["set", "admin"])
        assert count == 2

        htpasswd.load(force=True)
        assert htpasswd.check_password("admin", _make_passwd("admin"))
        assert old_users == set(htpasswd.users())


# =====
def test_ok__del(htpasswd: passlib.apache.HtpasswdFile) -> None:
    old_users = set(htpasswd.users())

    if old_users:
        assert htpasswd.check_password("admin", _make_passwd("admin"))

    _run_htpasswd(htpasswd, ["del", "admin"])

    htpasswd.load(force=True)
    assert not htpasswd.check_password("admin", _make_passwd("admin"))
    assert old_users.difference(["admin"]) == set(htpasswd.users())
