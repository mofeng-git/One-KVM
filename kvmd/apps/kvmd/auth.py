import secrets

from typing import Dict
from typing import Optional

import passlib.apache

from ...logging import get_logger


# =====
class AuthManager:
    def __init__(self, htpasswd_path: str) -> None:
        self.__htpasswd_path = htpasswd_path
        self.__tokens: Dict[str, str] = {}  # {token: user}

    def login(self, user: str, passwd: str) -> Optional[str]:
        htpasswd = passlib.apache.HtpasswdFile(self.__htpasswd_path)
        if htpasswd.check_password(user, passwd):
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
