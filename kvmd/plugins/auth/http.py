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


import aiohttp
import aiohttp.web

from ...yamlconf import Option

from ...validators.basic import valid_bool
from ...validators.basic import valid_float_f01

from ...logging import get_logger

from ... import htclient

from . import BaseAuthService


# =====
class Plugin(BaseAuthService):
    def __init__(  # pylint: disable=super-init-not-called
        self,
        url: str,
        verify: bool,
        secret: str,
        user: str,
        passwd: str,
        timeout: float,
    ) -> None:

        self.__url = url
        self.__verify = verify
        self.__secret = secret
        self.__user = user
        self.__passwd = passwd
        self.__timeout = timeout

        self.__http_session: (aiohttp.ClientSession | None) = None

    @classmethod
    def get_plugin_options(cls) -> dict:
        return {
            "url":     Option("http://localhost/auth"),
            "verify":  Option(True, type=valid_bool),
            "secret":  Option(""),
            "user":    Option(""),
            "passwd":  Option(""),
            "timeout": Option(5.0, type=valid_float_f01),
        }

    async def authorize(self, user: str, passwd: str) -> bool:
        assert user == user.strip()
        assert user
        session = self.__ensure_http_session()
        try:
            async with session.request(
                method="POST",
                url=self.__url,
                timeout=self.__timeout,
                json={
                    "user": user,
                    "passwd": passwd,
                    "secret": self.__secret,
                },
                headers={
                    "User-Agent": htclient.make_user_agent("KVMD"),
                    "X-KVMD-User": user,
                },
            ) as response:
                htclient.raise_not_200(response)
                return True
        except Exception:
            get_logger().exception("Failed HTTP auth request for user %r", user)
            return False

    async def cleanup(self) -> None:
        if self.__http_session:
            await self.__http_session.close()
            self.__http_session = None

    def __ensure_http_session(self) -> aiohttp.ClientSession:
        if not self.__http_session:
            kwargs: dict = {}
            if self.__user:
                kwargs["auth"] = aiohttp.BasicAuth(login=self.__user, password=self.__passwd)
            if not self.__verify:
                kwargs["connector"] = aiohttp.TCPConnector(ssl=False)
            self.__http_session = aiohttp.ClientSession(**kwargs)
        return self.__http_session
