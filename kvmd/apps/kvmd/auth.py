# ========================================================================== #
#                                                                            #
#    KVMD - The main PiKVM daemon.                                           #
#                                                                            #
#    Copyright (C) 2018-2022  Maxim Devaev <mdevaev@gmail.com>               #
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
import pyotp

from ...logging import get_logger

from ... import aiotools

from ...plugins.auth import BaseAuthService
from ...plugins.auth import get_auth_service_class


# =====
class AuthManager:
    def __init__(
        self,
        enabled: bool,

        internal_type: str,
        internal_kwargs: dict,
        force_internal_users: list[str],

        external_type: str,
        external_kwargs: dict,

        totp_secret_path: str,
    ) -> None:

        self.__enabled = enabled
        if not enabled:
            get_logger().warning("AUTHORIZATION IS DISABLED")

        self.__internal_service: (BaseAuthService | None) = None
        if enabled:
            self.__internal_service = get_auth_service_class(internal_type)(**internal_kwargs)
            get_logger().info("Using internal auth service %r", self.__internal_service.get_plugin_name())

        self.__force_internal_users = force_internal_users

        self.__external_service: (BaseAuthService | None) = None
        if enabled and external_type:
            self.__external_service = get_auth_service_class(external_type)(**external_kwargs)
            get_logger().info("Using external auth service %r", self.__external_service.get_plugin_name())

        self.__totp_secret_path = totp_secret_path

        self.__tokens: dict[str, str] = {}  # {token: user}

    def is_auth_enabled(self) -> bool:
        return self.__enabled

    async def authorize(self, user: str, passwd: str) -> bool:
        assert user == user.strip()
        assert user
        assert self.__enabled
        assert self.__internal_service

        if self.__totp_secret_path:
            with open(self.__totp_secret_path) as secret_file:
                secret = secret_file.read().strip()
            if secret:
                code = passwd[-6:]
                if not pyotp.TOTP(secret).verify(code):
                    get_logger().error("Got access denied for user %r by TOTP", user)
                    return False
                passwd = passwd[:-6]

        if user not in self.__force_internal_users and self.__external_service:
            service = self.__external_service
        else:
            service = self.__internal_service

        ok = (await service.authorize(user, passwd))
        if ok:
            get_logger().info("Authorized user %r via auth service %r", user, service.get_plugin_name())
        else:
            get_logger().error("Got access denied for user %r from auth service %r", user, service.get_plugin_name())
        return ok

    async def login(self, user: str, passwd: str) -> (str | None):
        assert user == user.strip()
        assert user
        assert self.__enabled
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
        assert self.__enabled
        user = self.__tokens.pop(token, "")
        if user:
            get_logger().info("Logged out user %r", user)

    def check(self, token: str) -> (str | None):
        assert self.__enabled
        return self.__tokens.get(token)

    @aiotools.atomic_fg
    async def cleanup(self) -> None:
        if self.__enabled:
            assert self.__internal_service
            await self.__internal_service.cleanup()
            if self.__external_service:
                await self.__external_service.cleanup()
