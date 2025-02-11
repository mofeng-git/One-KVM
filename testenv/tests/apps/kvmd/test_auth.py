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


import os
import asyncio
import contextlib

from typing import AsyncGenerator

import pytest

from kvmd.yamlconf import make_config

from kvmd.apps.kvmd.auth import AuthManager

from kvmd.plugins.auth import get_auth_service_class

from kvmd.htserver import HttpExposed

from kvmd.crypto import KvmdHtpasswdFile


# =====
_E_AUTH = HttpExposed("GET", "/foo_auth", True, (lambda: None))
_E_UNAUTH = HttpExposed("GET", "/bar_unauth", True, (lambda: None))
_E_FREE = HttpExposed("GET", "/baz_free", False, (lambda: None))


def _make_service_kwargs(path: str) -> dict:
    cls = get_auth_service_class("htpasswd")
    scheme = cls.get_plugin_options()
    return make_config({"file": path}, scheme)._unpack()


@contextlib.asynccontextmanager
async def _get_configured_manager(
    unauth_paths: list[str],
    int_path: str,
    ext_path: str="",
    force_int_users: (list[str] | None)=None,
) -> AsyncGenerator[AuthManager, None]:

    manager = AuthManager(
        enabled=True,
        expire=0,
        unauth_paths=unauth_paths,

        int_type="htpasswd",
        int_kwargs=_make_service_kwargs(int_path),
        force_int_users=(force_int_users or []),

        ext_type=("htpasswd" if ext_path else ""),
        ext_kwargs=(_make_service_kwargs(ext_path) if ext_path else {}),

        totp_secret_path="",
    )

    try:
        yield manager
    finally:
        await manager.cleanup()


# =====
@pytest.mark.asyncio
async def test_ok__expire(tmpdir) -> None:  # type: ignore
    path = os.path.abspath(str(tmpdir.join("htpasswd")))

    htpasswd = KvmdHtpasswdFile(path, new=True)
    htpasswd.set_password("admin", "pass")
    htpasswd.save()

    async with _get_configured_manager([], path) as manager:
        assert manager.is_auth_enabled()
        assert manager.is_auth_required(_E_AUTH)
        assert manager.is_auth_required(_E_UNAUTH)
        assert not manager.is_auth_required(_E_FREE)

        assert manager.check("xxx") is None
        manager.logout("xxx")

        assert (await manager.login("user", "foo", 3)) is None
        assert (await manager.login("admin", "foo", 3)) is None
        assert (await manager.login("user", "pass", 3)) is None

        token1 = await manager.login("admin", "pass", 3)
        assert isinstance(token1, str)
        assert len(token1) == 64

        token2 = await manager.login("admin", "pass", 3)
        assert isinstance(token2, str)
        assert len(token2) == 64
        assert token1 != token2

        assert manager.check(token1) == "admin"
        assert manager.check(token2) == "admin"
        assert manager.check("foobar") is None

        manager.logout(token1)

        assert manager.check(token1) is None
        assert manager.check(token2) is None
        assert manager.check("foobar") is None

        token3 = await manager.login("admin", "pass", 3)
        assert isinstance(token3, str)
        assert len(token3) == 64
        assert token1 != token3
        assert token2 != token3

        token4 = await manager.login("admin", "pass", 6)
        assert isinstance(token4, str)
        assert len(token4) == 64
        assert token1 != token4
        assert token2 != token4
        assert token3 != token4

        await asyncio.sleep(4)

        assert manager.check(token1) is None
        assert manager.check(token2) is None
        assert manager.check(token3) is None
        assert manager.check(token4) == "admin"

        await asyncio.sleep(3)

        assert manager.check(token1) is None
        assert manager.check(token2) is None
        assert manager.check(token3) is None
        assert manager.check(token4) is None


@pytest.mark.asyncio
async def test_ok__internal(tmpdir) -> None:  # type: ignore
    path = os.path.abspath(str(tmpdir.join("htpasswd")))

    htpasswd = KvmdHtpasswdFile(path, new=True)
    htpasswd.set_password("admin", "pass")
    htpasswd.save()

    async with _get_configured_manager([], path) as manager:
        assert manager.is_auth_enabled()
        assert manager.is_auth_required(_E_AUTH)
        assert manager.is_auth_required(_E_UNAUTH)
        assert not manager.is_auth_required(_E_FREE)

        assert manager.check("xxx") is None
        manager.logout("xxx")

        assert (await manager.login("user", "foo", 0)) is None
        assert (await manager.login("admin", "foo", 0)) is None
        assert (await manager.login("user", "pass", 0)) is None

        token1 = await manager.login("admin", "pass", 0)
        assert isinstance(token1, str)
        assert len(token1) == 64

        token2 = await manager.login("admin", "pass", 0)
        assert isinstance(token2, str)
        assert len(token2) == 64
        assert token1 != token2

        assert manager.check(token1) == "admin"
        assert manager.check(token2) == "admin"
        assert manager.check("foobar") is None

        manager.logout(token1)

        assert manager.check(token1) is None
        assert manager.check(token2) is None
        assert manager.check("foobar") is None

        token3 = await manager.login("admin", "pass", 0)
        assert isinstance(token3, str)
        assert len(token3) == 64
        assert token1 != token3
        assert token2 != token3


@pytest.mark.asyncio
async def test_ok__external(tmpdir) -> None:  # type: ignore
    path1 = os.path.abspath(str(tmpdir.join("htpasswd1")))
    path2 = os.path.abspath(str(tmpdir.join("htpasswd2")))

    htpasswd1 = KvmdHtpasswdFile(path1, new=True)
    htpasswd1.set_password("admin", "pass1")
    htpasswd1.set_password("local", "foobar")
    htpasswd1.save()

    htpasswd2 = KvmdHtpasswdFile(path2, new=True)
    htpasswd2.set_password("admin", "pass2")
    htpasswd2.set_password("user", "foobar")
    htpasswd2.save()

    async with _get_configured_manager([], path1, path2, ["admin"]) as manager:
        assert manager.is_auth_enabled()
        assert manager.is_auth_required(_E_AUTH)
        assert manager.is_auth_required(_E_UNAUTH)
        assert not manager.is_auth_required(_E_FREE)

        assert (await manager.login("local", "foobar", 0)) is None
        assert (await manager.login("admin", "pass2", 0)) is None

        token = await manager.login("admin", "pass1", 0)
        assert token is not None

        assert manager.check(token) == "admin"
        manager.logout(token)
        assert manager.check(token) is None

        token = await manager.login("user", "foobar", 0)
        assert token is not None

        assert manager.check(token) == "user"
        manager.logout(token)
        assert manager.check(token) is None


@pytest.mark.asyncio
async def test_ok__unauth(tmpdir) -> None:  # type: ignore
    path = os.path.abspath(str(tmpdir.join("htpasswd")))

    htpasswd = KvmdHtpasswdFile(path, new=True)
    htpasswd.set_password("admin", "pass")
    htpasswd.save()

    async with _get_configured_manager([
        "", " ",
        "foo_auth", "/foo_auth ", " /foo_auth",
        "/foo_authx", "/foo_auth/", "/foo_auth/x",
        "/bar_unauth",  # Only this one is matching
    ], path) as manager:

        assert manager.is_auth_enabled()
        assert manager.is_auth_required(_E_AUTH)
        assert not manager.is_auth_required(_E_UNAUTH)
        assert not manager.is_auth_required(_E_FREE)


@pytest.mark.asyncio
async def test_ok__disabled() -> None:
    try:
        manager = AuthManager(
            enabled=False,
            expire=0,
            unauth_paths=[],

            int_type="foobar",
            int_kwargs={},
            force_int_users=[],

            ext_type="",
            ext_kwargs={},

            totp_secret_path="",
        )

        assert not manager.is_auth_enabled()
        assert not manager.is_auth_required(_E_AUTH)
        assert not manager.is_auth_required(_E_UNAUTH)
        assert not manager.is_auth_required(_E_FREE)

        with pytest.raises(AssertionError):
            await manager.authorize("admin", "admin")

        with pytest.raises(AssertionError):
            await manager.login("admin", "admin", 0)

        with pytest.raises(AssertionError):
            manager.logout("xxx")

        with pytest.raises(AssertionError):
            manager.check("xxx")
    finally:
        await manager.cleanup()
