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


import dataclasses
import time
import datetime

import secrets
import pyotp

from ...logging import get_logger

from ... import aiotools

from ...plugins.auth import BaseAuthService
from ...plugins.auth import get_auth_service_class

from ...htserver import HttpExposed


# =====
@dataclasses.dataclass(frozen=True)
class _Session:
    user:      str
    expire_ts: int

    def __post_init__(self) -> None:
        assert self.user == self.user.strip()
        assert self.user
        assert self.expire_ts >= 0


class AuthManager:  # pylint: disable=too-many-instance-attributes
    def __init__(
        self,
        enabled: bool,
        expire: int,
        unauth_paths: list[str],

        int_type: str,
        int_kwargs: dict,
        force_int_users: list[str],

        ext_type: str,
        ext_kwargs: dict,

        totp_secret_path: str,
    ) -> None:

        logger = get_logger(0)

        self.__enabled = enabled
        if not enabled:
            logger.warning("AUTHORIZATION IS DISABLED")

        assert expire >= 0
        self.__expire = expire
        if expire > 0:
            logger.info("Maximum user session time is limited: %s",
                        self.__format_seconds(expire))

        self.__unauth_paths = frozenset(unauth_paths)  # To speed up
        for path in self.__unauth_paths:
            logger.warning("Authorization is disabled for API %r", path)

        self.__int_service: (BaseAuthService | None) = None
        if enabled:
            self.__int_service = get_auth_service_class(int_type)(**int_kwargs)
            logger.info("Using internal auth service %r",
                        self.__int_service.get_plugin_name())

        self.__force_int_users = force_int_users

        self.__ext_service: (BaseAuthService | None) = None
        if enabled and ext_type:
            self.__ext_service = get_auth_service_class(ext_type)(**ext_kwargs)
            logger.info("Using external auth service %r",
                        self.__ext_service.get_plugin_name())

        self.__totp_secret_path = totp_secret_path

        self.__sessions: dict[str, _Session] = {}  # {token: session}

    def is_auth_enabled(self) -> bool:
        return self.__enabled

    def is_auth_required(self, exposed: HttpExposed) -> bool:
        return (
            self.is_auth_enabled()
            and exposed.auth_required
            and exposed.path not in self.__unauth_paths
        )

    async def authorize(self, user: str, passwd: str) -> bool:
        assert user == user.strip()
        assert user
        assert self.__enabled
        assert self.__int_service
        logger = get_logger(0)

        if self.__totp_secret_path:
            with open(self.__totp_secret_path) as file:
                secret = file.read().strip()
            if secret:
                code = passwd[-6:]
                if not pyotp.TOTP(secret).verify(code, valid_window=1):
                    logger.error("Got access denied for user %r by TOTP", user)
                    return False
                passwd = passwd[:-6]

        if user not in self.__force_int_users and self.__ext_service:
            service = self.__ext_service
        else:
            service = self.__int_service

        pname = service.get_plugin_name()
        ok = (await service.authorize(user, passwd))
        if ok:
            logger.info("Authorized user %r via auth service %r", user, pname)
        else:
            logger.error("Got access denied for user %r from auth service %r", user, pname)
        return ok

    async def login(self, user: str, passwd: str, expire: int) -> (str | None):
        assert user == user.strip()
        assert user
        assert expire >= 0
        assert self.__enabled

        if (await self.authorize(user, passwd)):
            token = self.__make_new_token()
            session = _Session(
                user=user,
                expire_ts=self.__make_expire_ts(expire),
            )
            self.__sessions[token] = session
            get_logger(0).info("Logged in user %r; expire=%s, sessions_now=%d",
                               session.user,
                               self.__format_expire_ts(session.expire_ts),
                               self.__get_sessions_number(session.user))
            return token

        return None

    def __make_new_token(self) -> str:
        for _ in range(10):
            token = secrets.token_hex(32)
            if token not in self.__sessions:
                return token
        raise RuntimeError("Can't generate new unique token")

    def __make_expire_ts(self, expire: int) -> int:
        assert expire >= 0
        assert self.__expire >= 0

        if expire == 0:
            # The user requested infinite session: apply global expire.
            # It will allow this (0) or set a limit.
            expire = self.__expire
        else:
            # The user wants a limited session
            if self.__expire > 0:
                # If we have a global limit, override the user limit
                assert expire > 0
                expire = min(expire, self.__expire)

        if expire > 0:
            return (self.__get_now_ts() + expire)

        assert expire == 0
        return 0

    def __get_now_ts(self) -> int:
        return int(time.monotonic())

    def __format_expire_ts(self, expire_ts: int) -> str:
        if expire_ts > 0:
            seconds = expire_ts - self.__get_now_ts()
            return f"[{self.__format_seconds(seconds)}]"
        return "INF"

    def __format_seconds(self, seconds: int) -> str:
        return str(datetime.timedelta(seconds=seconds))

    def __get_sessions_number(self, user: str) -> int:
        return sum(
            1
            for session in self.__sessions.values()
            if session.user == user
        )

    def logout(self, token: str) -> None:
        assert self.__enabled
        if token in self.__sessions:
            user = self.__sessions[token].user
            count = 0
            for (key_t, session) in list(self.__sessions.items()):
                if session.user == user:
                    count += 1
                    del self.__sessions[key_t]
            get_logger(0).info("Logged out user %r; sessions_closed=%d", user, count)

    def check(self, token: str) -> (str | None):
        assert self.__enabled
        session = self.__sessions.get(token)
        if session is not None:
            if session.expire_ts <= 0:
                # Infinite session
                return session.user
            else:
                # Limited session
                if self.__get_now_ts() < session.expire_ts:
                    return session.user
                else:
                    del self.__sessions[token]
                    get_logger(0).info("The session of user %r is expired; sessions_left=%d",
                                       session.user,
                                       self.__get_sessions_number(session.user))
        return None

    @aiotools.atomic_fg
    async def cleanup(self) -> None:
        if self.__enabled:
            assert self.__int_service
            await self.__int_service.cleanup()
            if self.__ext_service:
                await self.__ext_service.cleanup()
