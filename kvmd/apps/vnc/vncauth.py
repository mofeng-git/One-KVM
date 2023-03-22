# ========================================================================== #
#                                                                            #
#    KVMD - The main PiKVM daemon.                                           #
#                                                                            #
#    Copyright (C) 2020  Maxim Devaev <mdevaev@gmail.com>                    #
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

from ...logging import get_logger

from ... import aiotools


# =====
class VncAuthError(Exception):
    def __init__(self, path: str, lineno: int, msg: str) -> None:
        super().__init__(f"Syntax error at {path}:{lineno}: {msg}")


# =====
@dataclasses.dataclass(frozen=True)
class VncAuthKvmdCredentials:
    user: str
    passwd: str


class VncAuthManager:
    def __init__(
        self,
        path: str,
        enabled: bool,
    ) -> None:

        self.__path = path
        self.__enabled = enabled

    async def read_credentials(self) -> tuple[dict[str, VncAuthKvmdCredentials], bool]:
        if self.__enabled:
            try:
                return (await self.__inner_read_credentials(), True)
            except VncAuthError as err:
                get_logger(0).error(str(err))
            except Exception:
                get_logger(0).exception("Unhandled exception while reading VNCAuth passwd file")
        return ({}, (not self.__enabled))

    async def __inner_read_credentials(self) -> dict[str, VncAuthKvmdCredentials]:
        lines = (await aiotools.read_file(self.__path)).split("\n")
        credentials: dict[str, VncAuthKvmdCredentials] = {}
        for (lineno, line) in enumerate(lines):
            if len(line.strip()) == 0 or line.lstrip().startswith("#"):
                continue

            if " -> " not in line:
                raise VncAuthError(self.__path, lineno, "Missing ' -> ' operator")

            (vnc_passwd, kvmd_userpass) = map(str.lstrip, line.split(" -> ", 1))
            if ":" not in kvmd_userpass:
                raise VncAuthError(self.__path, lineno, "Missing ':' operator in KVMD credentials (right part)")

            (kvmd_user, kvmd_passwd) = kvmd_userpass.split(":")
            kvmd_user = kvmd_user.strip()
            if len(kvmd_user) == 0:
                raise VncAuthError(self.__path, lineno, "Empty KVMD user (right part)")

            if vnc_passwd in credentials:
                raise VncAuthError(self.__path, lineno, "Duplicating VNC password (left part)")

            credentials[vnc_passwd] = VncAuthKvmdCredentials(kvmd_user, kvmd_passwd)
        return credentials
