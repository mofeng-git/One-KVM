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


from typing import Dict

import passlib.apache

from ...yamlconf import Option

from ...validators.fs import valid_abs_path_exists

from . import BaseAuthService


# =====
class Plugin(BaseAuthService):
    PLUGIN_NAME = "htpasswd"

    def __init__(self, path: str) -> None:  # pylint: disable=super-init-not-called
        self.__path = path

    @classmethod
    def get_options(cls) -> Dict[str, Option]:
        return {
            "file": Option("/etc/kvmd/htpasswd", type=valid_abs_path_exists, unpack_as="path"),
        }

    async def login(self, user: str, passwd: str) -> bool:
        htpasswd = passlib.apache.HtpasswdFile(self.__path)
        return htpasswd.check_password(user, passwd)
