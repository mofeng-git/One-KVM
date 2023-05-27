# ========================================================================== #
#                                                                            #
#    KVMD - The main PiKVM daemon.                                           #
#                                                                            #
#    Copyright (C) 2018-2023  Maxim Devaev <mdevaev@gmail.com>               #
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


import asyncio
import pwd

import pam

from ...yamlconf import Option

from ...validators.basic import valid_int_f0
from ...validators.auth import valid_users_list

from ...logging import get_logger

from ... import aiotools

from . import BaseAuthService


# =====
class Plugin(BaseAuthService):
    def __init__(  # pylint: disable=super-init-not-called
        self,
        service: str,
        allow_users: list[str],
        deny_users: list[str],
        allow_uids_at: int,
    ) -> None:

        self.__service = service
        self.__allow_users = allow_users
        self.__deny_users = deny_users
        self.__allow_uids_at = allow_uids_at

        self.__lock = asyncio.Lock()

    @classmethod
    def get_plugin_options(cls) -> dict:
        return {
            "service":       Option("login"),
            "allow_users":   Option([], type=valid_users_list),
            "deny_users":    Option([], type=valid_users_list),
            "allow_uids_at": Option(0,  type=valid_int_f0),
        }

    async def authorize(self, user: str, passwd: str) -> bool:
        assert user == user.strip()
        assert user
        async with self.__lock:
            return (await aiotools.run_async(self.__inner_authorize, user, passwd))

    def __inner_authorize(self, user: str, passwd: str) -> bool:
        if self.__allow_users and user not in self.__allow_users:
            get_logger().error("User %r not in allow-list", user)
            return False

        if self.__deny_users and user in self.__deny_users:
            get_logger().error("User %r in deny-list", user)
            return False

        if self.__allow_uids_at > 0:
            try:
                uid = pwd.getpwnam(user).pw_uid
            except Exception:
                get_logger().exception("Can't find UID of user %r", user)
                return False
            else:
                if uid < self.__allow_uids_at:
                    get_logger().error("Unallowed UID of user %r: uid=%d < allow_uids_at=%d",
                                       user, uid, self.__allow_uids_at)
                    return False

        pam_obj = pam.pam()
        if not pam_obj.authenticate(user, passwd, service=self.__service):
            get_logger().error("Can't authorize user %r using PAM: code=%d; reason=%s",
                               user, pam_obj.code, pam_obj.reason)
            return False
        return True
