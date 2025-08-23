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


import threading
import functools
import time

from ...logging import get_logger

from ... import tools


# =====
class IpmiPasswdError(Exception):
    def __init__(self, path: str, lineno: int, msg: str) -> None:
        super().__init__(f"Syntax error at {path}:{lineno}: {msg}")


class IpmiAuthManager:
    def __init__(self, path: str) -> None:
        self.__path = path
        self.__lock = threading.Lock()

    def get(self, user: str) -> (str | None):
        creds = self.__get_credentials(int(time.time()))
        return creds.get(user)

    @functools.lru_cache(maxsize=1)
    def __get_credentials(self, ts: int) -> dict[str, str]:
        _ = ts
        with self.__lock:
            try:
                return self.__read_credentials()
            except Exception as ex:
                get_logger().error("%s", tools.efmt(ex))
            return {}

    def __read_credentials(self) -> dict[str, str]:
        with open(self.__path) as file:
            creds: dict[str, str] = {}
            for (lineno, line) in tools.passwds_splitted(file.read()):
                if " -> " in line:  # Compatibility with old ipmipasswd file format
                    line = line.split(" -> ", 1)[0]

                if ":" not in line:
                    raise IpmiPasswdError(self.__path, lineno, "Missing ':' operator")

                (user, passwd) = line.split(":", 1)
                user = user.strip()
                if len(user) == 0:
                    raise IpmiPasswdError(self.__path, lineno, "Empty IPMI user")

                if user in creds:
                    raise IpmiPasswdError(self.__path, lineno, f"Found duplicating user {user!r}")

                creds[user] = passwd
            return creds
