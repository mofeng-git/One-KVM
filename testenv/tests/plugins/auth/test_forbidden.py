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


import pytest

from . import get_configured_auth_service


# =====
@pytest.mark.asyncio
async def test_ok__forbidden_service() -> None:  # type: ignore
    async with get_configured_auth_service("forbidden") as service:
        assert not (await service.authorize("user", "foo"))
        assert not (await service.authorize("admin", "foo"))
        assert not (await service.authorize("user", "pass"))
        assert not (await service.authorize("admin", "pass"))
        assert not (await service.authorize("admin", "admin"))
        assert not (await service.authorize("admin", ""))
        assert not (await service.authorize("", ""))
