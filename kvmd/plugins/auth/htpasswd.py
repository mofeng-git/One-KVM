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


import passlib.apache

from ...yamlconf import Option

from ...validators.os import valid_abs_file

from . import BaseAuthService


# =====
class Plugin(BaseAuthService):
    def __init__(self, path: str) -> None:  # pylint: disable=super-init-not-called
        self.__path = path

    @classmethod
    def get_plugin_options(cls) -> dict:
        return {
            "file": Option("/etc/kvmd/htpasswd", type=valid_abs_file, unpack_as="path"),
        }

    async def authorize(self, user: str, passwd: str) -> bool:
        assert user == user.strip()
        assert user
        htpasswd = passlib.apache.HtpasswdFile(self.__path)
        return htpasswd.check_password(user, passwd)
