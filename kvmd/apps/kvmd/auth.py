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

        internal_type: str,
        internal_kwargs: Dict,

        external_type: str,
        external_kwargs: Dict,

        internal_users: List[str],
    ) -> None:

        self.__internal_service = get_auth_service_class(internal_type)(**internal_kwargs)
        get_logger().info("Using internal auth service %r", self.__internal_service.get_plugin_name())

        self.__external_service: Optional[BaseAuthService] = None
        if external_type:
            self.__external_service = get_auth_service_class(external_type)(**external_kwargs)
            get_logger().info("Using external auth service %r", self.__external_service.get_plugin_name())

        self.__internal_users = internal_users

        self.__tokens: Dict[str, str] = {}  # {token: user}

    async def authorize(self, user: str, passwd: str) -> bool:
        if user not in self.__internal_users and self.__external_service:
            service = self.__external_service
        else:
            service = self.__internal_service

        ok = (await service.authorize(user, passwd))
        if ok:
            get_logger().info("Authorized user %r via auth service %r", user, service.get_plugin_name())
        else:
            get_logger().error("Got access denied for user %r from auth service %r", user, service.get_plugin_name())
        return ok

    async def login(self, user: str, passwd: str) -> Optional[str]:
        if (await self.authorize(user, passwd)):
            for (token, token_user) in self.__tokens.items():
                if user == token_user:
                    return token
            token = secrets.token_hex(32)
            self.__tokens[token] = user
            get_logger().info("Logged in user %r", user)
            return token
        else:
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
