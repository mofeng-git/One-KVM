# ========================================================================== #
#                                                                            #
#    KVMD - The main Pi-KVM daemon.                                          #
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

from typing import Tuple
from typing import Dict

import aiofiles

from ...logging import get_logger


# =====
class VncAuthError(Exception):
    def __init__(self, msg: str) -> None:
        super().__init__(f"Incorrect VNCAuth passwd file: {msg}")


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

    async def read_credentials(self) -> Tuple[Dict[str, VncAuthKvmdCredentials], bool]:
        if self.__enabled:
            try:
                return (await self.__inner_read_credentials(), True)
            except VncAuthError as err:
                get_logger(0).error(str(err))
            except Exception:
                get_logger(0).exception("Unhandled exception while reading VNCAuth passwd file")
        return ({}, (not self.__enabled))

    async def __inner_read_credentials(self) -> Dict[str, VncAuthKvmdCredentials]:
        async with aiofiles.open(self.__path) as vc_file:
            lines = (await vc_file.read()).split("\n")

        credentials: Dict[str, VncAuthKvmdCredentials] = {}
        for (number, line) in enumerate(lines):
            if len(line.strip()) == 0 or line.lstrip().startswith("#"):
                continue

            if " -> " not in line:
                raise VncAuthError(f"Missing ' -> ' operator at line #{number}")

            (vnc_passwd, kvmd_userpass) = map(str.lstrip, line.split(" -> ", 1))
            if ":" not in kvmd_userpass:
                raise VncAuthError(f"Missing ':' operator in KVMD credentials (right part) at line #{number}")

            (kvmd_user, kvmd_passwd) = kvmd_userpass.split(":")
            kvmd_user = kvmd_user.strip()

            if vnc_passwd in credentials:
                raise VncAuthError(f"Found duplicating VNC password (left part) at line #{number}")

            credentials[vnc_passwd] = VncAuthKvmdCredentials(kvmd_user, kvmd_passwd)
        return credentials
