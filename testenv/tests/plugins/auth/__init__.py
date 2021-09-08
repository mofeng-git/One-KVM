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


import contextlib

from typing import AsyncGenerator
from typing import Any

from kvmd.yamlconf import make_config

from kvmd.plugins.auth import BaseAuthService
from kvmd.plugins.auth import get_auth_service_class


# =====
@contextlib.asynccontextmanager
async def get_configured_auth_service(name: str, **kwargs: Any) -> AsyncGenerator[BaseAuthService, None]:
    service_class = get_auth_service_class(name)
    config = make_config(kwargs, service_class.get_plugin_options())
    service = service_class(**config._unpack())
    try:
        yield service
    finally:
        await service.cleanup()
