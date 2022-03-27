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


import io
import textwrap

from typing import Dict

import pyrad.client
import pyrad.packet
import pyrad.dictionary

from ...yamlconf import Option

from ...validators.net import valid_port
from ...validators.net import valid_ip_or_host
from ...validators.basic import valid_int_f1

from ...logging import get_logger

from ... import aiotools

from . import BaseAuthService


# =====
_FREERADUIS_DICT = textwrap.dedent("""
    # https://github.com/pyradius/pyrad/raw/master/example/dictionary.freeradius

    VENDOR			FreeRADIUS			11344
    BEGIN-VENDOR	FreeRADIUS

    ATTRIBUTE	FreeRADIUS-Statistics-Type		127	integer
    ATTRIBUTE   User-Name       1   string
    ATTRIBUTE   User-Password       2   string

    VALUE	FreeRADIUS-Statistics-Type	None					0
    VALUE	FreeRADIUS-Statistics-Type	Authentication			1
    VALUE	FreeRADIUS-Statistics-Type	Accounting				2
    VALUE	FreeRADIUS-Statistics-Type	Proxy-Authentication	4
    VALUE	FreeRADIUS-Statistics-Type	Proxy-Accounting		8
    VALUE	FreeRADIUS-Statistics-Type	Internal				0x10
    VALUE	FreeRADIUS-Statistics-Type	Client					0x20
    VALUE	FreeRADIUS-Statistics-Type	Server					0x40
    VALUE	FreeRADIUS-Statistics-Type	Home-Server				0x80

    VALUE	FreeRADIUS-Statistics-Type	Auth-Acct				0x03
    VALUE	FreeRADIUS-Statistics-Type	Proxy-Auth-Acct			0x0c

    VALUE	FreeRADIUS-Statistics-Type	All						0x1f

    END-VENDOR FreeRADIUS
""")


# =====
class Plugin(BaseAuthService):
    def __init__(  # pylint: disable=super-init-not-called
        self,
        host: str,
        port: int,
        secret: str,
        timeout: float,
    ) -> None:

        self.__host = host
        self.__port = port
        self.__secret = secret
        self.__timeout = timeout

    @classmethod
    def get_plugin_options(cls) -> Dict:
        return {
            "host":    Option("localhost", type=valid_ip_or_host),
            "port":    Option(1812, type=valid_port),
            "secret":  Option(""),
            "timeout": Option(5, type=valid_int_f1),
        }

    async def authorize(self, user: str, passwd: str) -> bool:
        return (await aiotools.run_async(self.__inner_authorize, user, passwd))

    def __inner_authorize(self, user: str, passwd: str) -> bool:
        assert user == user.strip()
        assert user
        try:
            with io.StringIO(_FREERADUIS_DICT) as dct_file:
                dct = pyrad.dictionary.Dictionary(dct_file)
            client = pyrad.client.Client(
                server=self.__host,
                authport=self.__port,
                secret=self.__secret.encode("ascii"),
                timeout=self.__timeout,
                dict=dct,
            )
            request = client.CreateAuthPacket(code=pyrad.packet.AccessRequest, User_Name=user)
            request["User-Password"] = request.PwCrypt(passwd)
            response = client.SendPacket(request)
            return (response.code == pyrad.packet.AccessAccept)
        except Exception:
            get_logger().exception("Failed RADIUS auth request for user %r", user)
            return False
