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


from typing import Dict
from typing import AsyncGenerator

import aiohttp.web
import aiohttp_basicauth

import pytest

from . import get_configured_auth_service


# =====
async def _handle_auth(request: aiohttp.web.BaseRequest) -> aiohttp.web.Response:
    status = 400
    if request.method == "POST":
        credentials = (await request.json())
        if credentials["user"] == "admin" and credentials["passwd"] == "pass":
            status = 200
    return aiohttp.web.Response(text=str(status), status=status)


@pytest.fixture(name="auth_server_port")
async def _auth_server_port_fixture(aiohttp_server) -> AsyncGenerator[int, None]:  # type: ignore
    auth = aiohttp_basicauth.BasicAuthMiddleware(
        username="server-admin",
        password="server-pass",
        force=False,
    )

    app = aiohttp.web.Application(middlewares=[auth])
    app.router.add_post("/auth", _handle_auth)
    app.router.add_post("/auth_plus_basic", auth.required(_handle_auth))

    server = await aiohttp_server(app)
    try:
        yield server.port
    finally:
        await server.close()


# =====
@pytest.mark.asyncio
@pytest.mark.parametrize("kwargs", [
    {},
    {"verify": False},
    {"user": "server-admin", "passwd": "server-pass"},
])
async def test_ok(auth_server_port: int, kwargs: Dict) -> None:
    url = "http://localhost:%d/%s" % (
        auth_server_port,
        ("auth_plus_basic" if kwargs.get("user") else "auth"),
    )
    async with get_configured_auth_service("http", url=url, **kwargs) as service:
        assert not (await service.authorize("user", "foobar"))
        assert not (await service.authorize("admin", "foobar"))
        assert not (await service.authorize("user", "pass"))
        assert (await service.authorize("admin", "pass"))
