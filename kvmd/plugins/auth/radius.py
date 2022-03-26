# ========================================================================== #
#                                                                            #
#    KVMD - The main PiKVM daemon.                                           #
#                                                                            #
#    Copyright (C) 2018-2022  Maxim Devaev <mdevaev@gmail.com>               #
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

#
# For some reason this needs the two following files in /
#  https://raw.githubusercontent.com/AndrewAubury/kvmd/master/kvmd/plugins/auth/radius.py
#  https://github.com/pyradius/pyrad/raw/master/example/dictionary.freeradius
#

from typing import Dict

from ...yamlconf import Option

from ...validators.os import valid_abs_file
from ...validators.net import valid_port
from ...validators.net import valid_ip_or_host
from ...validators.basic import valid_int_f1

from . import BaseAuthService

from pyrad.client import Client
from pyrad.dictionary import Dictionary
import pyrad.packet 


# =====
class Plugin(BaseAuthService):
    def __init__(  # pylint: disable=super-init-not-called
        self,
        host: str,
        port: int,
        secret: str,
        user: str,
        passwd: str,
        timeout: int,
    ) -> None:

        self.__host = host
        self.__port = port
        self.__secret = secret
        self.__user = user
        self.__passwd = passwd
        self.__timeout = timeout

    @classmethod
    def get_plugin_options(cls) -> Dict:
        return {
            "host":     Option("localhost",type=valid_ip_or_host),
            "port":  Option(1812,type=valid_port),
            "secret":  Option(""),
            "user":    Option(""),
            "passwd":  Option(""),
            "timeout": Option(5,type=valid_int_f1),
        }

    async def authorize(self, user: str, passwd: str) -> bool:
        user = user.strip()
        try:
            srv = Client(server=self.__host, secret=self.__secret.encode('ascii'), timeout=self.__timeout, dict=Dictionary("dictionary"))
            req = srv.CreateAuthPacket(code=pyrad.packet.AccessRequest, User_Name=user)
            req["User-Password"] = req.PwCrypt(passwd)
            # send request
            reply = srv.SendPacket(req)
            return (reply.code == pyrad.packet.AccessAccept)
        except:
            return False
