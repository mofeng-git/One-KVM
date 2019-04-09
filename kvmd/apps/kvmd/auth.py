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


import secrets

from typing import List
from typing import Dict
from typing import Optional

from ...logging import get_logger

from ...plugins.auth import BaseAuthService
from ...plugins.auth import get_auth_service_class


# =====
class AuthManager:
    def __init__(
        self,
        internal_users: List[str],

        internal_type: str,
        external_type: str,

        internal: Dict,
        external: Dict,
    ) -> None:

        self.__internal_users = internal_users
        self.__internal_service = get_auth_service_class(internal_type)(**internal)
        get_logger().info("Using internal login service %r", self.__internal_service.PLUGIN_NAME)

        self.__external_service: Optional[BaseAuthService] = None
        if external_type:
            self.__external_service = get_auth_service_class(external_type)(**external)
            get_logger().info("Using external login service %r", self.__external_service.PLUGIN_NAME)

        self.__tokens: Dict[str, str] = {}  # {token: user}

    async def login(self, user: str, passwd: str) -> Optional[str]:
        if user not in self.__internal_users and self.__external_service:
            service = self.__external_service
        else:
            service = self.__internal_service

        if (await service.login(user, passwd)):
            for (token, token_user) in self.__tokens.items():
                if user == token_user:
                    return token
            token = secrets.token_hex(32)
            self.__tokens[token] = user
            get_logger().info("Logged in user %r via login service %r", user, service.PLUGIN_NAME)
            return token
        else:
            get_logger().error("Access denied for user %r from login service %r", user, service.PLUGIN_NAME)
            return None

    def logout(self, token: str) -> None:
        user = self.__tokens.pop(token, "")
        if user:
            get_logger().info("Logged out user %r", user)

    def check(self, token: str) -> Optional[str]:
        return self.__tokens.get(token)

    async def cleanup(self) -> None:
        await self.__internal_service.cleanup()
        if self.__external_service:
            await self.__external_service.cleanup()
