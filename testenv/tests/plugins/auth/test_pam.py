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
import asyncio
import pwd

from typing import Dict
from typing import AsyncGenerator
from typing import Optional

import pytest

from . import get_configured_auth_service


# =====
_UID = 1500
_USER = "foobar"
_PASSWD = "query"


# =====
async def _run_process(cmd: str, input: Optional[str]=None) -> None:  # pylint: disable=redefined-builtin
    proc = await asyncio.create_subprocess_exec(
        *cmd.split(" "),
        stdin=(asyncio.subprocess.PIPE if input is not None else None),
        preexec_fn=os.setpgrp,
    )
    await proc.communicate(input.encode() if input is not None else None)
    assert proc.returncode == 0


@pytest.fixture(name="test_user")
async def _test_user() -> AsyncGenerator[None, None]:
    with pytest.raises(KeyError):
        pwd.getpwnam(_USER)
    await _run_process(f"useradd -u {_UID} -s /bin/bash {_USER}")
    await _run_process("chpasswd", input=f"{_USER}:{_PASSWD}\n")

    assert pwd.getpwnam(_USER).pw_uid == _UID

    try:
        yield
    finally:
        await _run_process(f"userdel -r {_USER}")
        with pytest.raises(KeyError):
            pwd.getpwnam(_USER)


# =====
@pytest.mark.asyncio
@pytest.mark.parametrize("kwargs", [
    {},
    {"allow_users": [_USER]},
    {"allow_uids_at": _UID},
])
async def test_ok(test_user, kwargs: Dict) -> None:  # type: ignore
    _ = test_user
    async with get_configured_auth_service("pam", **kwargs) as service:
        assert not (await service.authorize(_USER, "invalid_password"))
        assert (await service.authorize(_USER, _PASSWD))


@pytest.mark.asyncio
@pytest.mark.parametrize("kwargs", [
    {"allow_users": ["root"]},
    {"deny_users": [_USER]},
    {"allow_uids_at": _UID + 1},
])
async def test_fail(test_user, kwargs: Dict) -> None:  # type: ignore
    _ = test_user
    async with get_configured_auth_service("pam", **kwargs) as service:
        assert not (await service.authorize(_USER, "invalid_password"))
        assert not (await service.authorize(_USER, _PASSWD))
