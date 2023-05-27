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


import dataclasses


# =====
class IpmiPasswdError(Exception):
    def __init__(self, path: str, lineno: int, msg: str) -> None:
        super().__init__(f"Syntax error at {path}:{lineno}: {msg}")


@dataclasses.dataclass(frozen=True)
class IpmiUserCredentials:
    ipmi_user: str
    ipmi_passwd: str
    kvmd_user: str
    kvmd_passwd: str


class IpmiAuthManager:
    def __init__(self, path: str) -> None:
        self.__path = path
        with open(path) as file:
            self.__credentials = self.__parse_passwd_file(file.read().split("\n"))

    def __contains__(self, ipmi_user: str) -> bool:
        return (ipmi_user in self.__credentials)

    def __getitem__(self, ipmi_user: str) -> str:
        return self.__credentials[ipmi_user].ipmi_passwd

    def get_credentials(self, ipmi_user: str) -> IpmiUserCredentials:
        return self.__credentials[ipmi_user]

    def __parse_passwd_file(self, lines: list[str]) -> dict[str, IpmiUserCredentials]:
        credentials: dict[str, IpmiUserCredentials] = {}
        for (lineno, line) in enumerate(lines):
            if len(line.strip()) == 0 or line.lstrip().startswith("#"):
                continue

            if " -> " not in line:
                raise IpmiPasswdError(self.__path, lineno, "Missing ' -> ' operator")

            (left, right) = map(str.lstrip, line.split(" -> ", 1))
            for (name, pair) in [("left", left), ("right", right)]:
                if ":" not in pair:
                    raise IpmiPasswdError(self.__path, lineno, f"Missing ':' operator in {name} credentials")

            (ipmi_user, ipmi_passwd) = left.split(":")
            ipmi_user = ipmi_user.strip()
            if len(ipmi_user) == 0:
                raise IpmiPasswdError(self.__path, lineno, "Empty IPMI user (left)")

            (kvmd_user, kvmd_passwd) = right.split(":")
            kvmd_user = kvmd_user.strip()
            if len(kvmd_user) == 0:
                raise IpmiPasswdError(self.__path, lineno, "Empty KVMD user (left)")

            if ipmi_user in credentials:
                raise IpmiPasswdError(self.__path, lineno, f"Found duplicating user {ipmi_user!r} (left)")

            credentials[ipmi_user] = IpmiUserCredentials(
                ipmi_user=ipmi_user,
                ipmi_passwd=ipmi_passwd,
                kvmd_user=kvmd_user,
                kvmd_passwd=kvmd_passwd,
            )
        return credentials
