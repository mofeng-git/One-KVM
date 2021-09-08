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

import passlib.apache

import pytest

from . import get_configured_auth_service


# =====
@pytest.mark.asyncio
async def test_ok__htpasswd_service(tmpdir) -> None:  # type: ignore
    path = os.path.abspath(str(tmpdir.join("htpasswd")))

    htpasswd = passlib.apache.HtpasswdFile(path, new=True)
    htpasswd.set_password("admin", "pass")
    htpasswd.save()

    async with get_configured_auth_service("htpasswd", file=path) as service:
        assert not (await service.authorize("user", "foo"))
        assert not (await service.authorize("admin", "foo"))
        assert not (await service.authorize("user", "pass"))
        assert (await service.authorize("admin", "pass"))

        htpasswd.set_password("admin", "bar")
        htpasswd.set_password("user", "bar")
        htpasswd.save()

        assert (await service.authorize("admin", "bar"))
        assert (await service.authorize("user", "bar"))
        assert not (await service.authorize("admin", "foo"))
        assert not (await service.authorize("user", "foo"))
