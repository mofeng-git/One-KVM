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

from typing import Dict
from typing import Optional

import passlib.apache

from ...logging import get_logger


# =====
class AuthManager:
    def __init__(self, auth_type: str, htpasswd: Dict) -> None:
        self.__login = {
            "htpasswd": lambda: _HtpasswdLogin(**htpasswd),
        }[auth_type]().login
        self.__tokens: Dict[str, str] = {}  # {token: user}

    def login(self, user: str, passwd: str) -> Optional[str]:
        if self.__login(user, passwd):
            for (token, token_user) in self.__tokens.items():
                if user == token_user:
                    return token
            token = secrets.token_hex(32)
            self.__tokens[token] = user
            get_logger().info("Logged in user %r", user)
            return token
        else:
            get_logger().error("Access denied for user %r", user)
            return None

    def logout(self, token: str) -> None:
        user = self.__tokens.pop(token, "")
        if user:
            get_logger().info("Logged out user %r", user)

    def check(self, token: str) -> Optional[str]:
        return self.__tokens.get(token)


class _HtpasswdLogin:
    def __init__(self, path: str) -> None:
        get_logger().info("Using htpasswd auth file %r", path)
        self.__path = path

    def login(self, user: str, passwd: str) -> bool:
        htpasswd = passlib.apache.HtpasswdFile(self.__path)
        return htpasswd.check_password(user, passwd)
