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


from typing import AsyncGenerator

import aiohttp.web

import pytest

from . import get_configured_auth_service


# =====
async def _handle_auth_post(request: aiohttp.web.BaseRequest) -> aiohttp.web.Response:
    status = 400
    if request.method == "POST":
        credentials = (await request.json())
        if credentials["user"] == "admin" and credentials["passwd"] == "foobar":
            status = 200
    return aiohttp.web.Response(text=str(status), status=status)


@pytest.fixture(name="auth_server_port")
async def _auth_server_port_fixture(aiohttp_server) -> AsyncGenerator[int, None]:  # type: ignore
    app = aiohttp.web.Application()
    app.router.add_post("/auth_post", _handle_auth_post)
    server = await aiohttp_server(app)
    try:
        yield server.port
    finally:
        await server.close()


# =====
@pytest.mark.asyncio
async def test_ok__http_service(auth_server_port: int) -> None:
    url = "http://localhost:%d/auth_post" % (auth_server_port)
    async with get_configured_auth_service("http", url=url) as service:
        assert not (await service.login("admin", "foo"))
        assert not (await service.login("user", "foo"))
        assert (await service.login("admin", "foobar"))
