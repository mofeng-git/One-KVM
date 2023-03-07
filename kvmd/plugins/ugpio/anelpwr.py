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

import requests
import aiohttp
import functools

from typing import Callable
from typing import Any

from ...logging import get_logger

from ... import tools
from ... import aiotools

from ...yamlconf import Option

from ...validators.hw import valid_number
from ...validators.basic import valid_stripped_string_not_empty

from . import UserGpioModes
from . import BaseUserGpioDriver
from . import GpioDriverOfflineError


# =====
class Plugin(BaseUserGpioDriver):  # pylint: disable=too-many-instance-attributes
    def __init__(  # pylint: disable=super-init-not-called
        self,
        instance_name: str,
        notifier: aiotools.AioNotifier,
        host: str,
        port: str,
        user: str,
        password: str,
        
    ) -> None:

        super().__init__(instance_name, notifier)

        self.__host = host
        self.__port = port
        self.__user = user
        self.__password = password

    @classmethod
    def get_plugin_options(cls) -> dict[str, Option]:
        return {
            "host":  Option([], type=valid_stripped_string_not_empty),
            "port":   Option([], type=valid_number),
            "user":   Option([], type=valid_stripped_string_not_empty),
            "password":   Option([], type=valid_stripped_string_not_empty),
        }

    @classmethod
    def get_modes(cls) -> set[str]:
        return set(UserGpioModes.ALL)

    @classmethod
    def get_pin_validator(cls) -> Callable[[Any], Any]:
        return  functools.partial(valid_number, min=1,max=8)

    async def read(self, pin: str) -> bool:
        try:
            #status = 0
            #body = ""
            async with aiohttp.ClientSession() as session:
                url  = f"http://{self.__host}:{self.__port}/strg.cfg"
                async with session.get(url,auth=aiohttp.BasicAuth(self.__user,self.__password)) as resp:
                    body = await resp.text()
                    if ( resp.status != 200 ):
                        get_logger(0).error(f"http get returned {resp.status} form {self.__host}")
                        raise GpioDriverOfflineError(self)

                    return '1' == body.split(';')[1 + (int(pin) - 1) * 5] 

        except Exception as e:
            get_logger(0).error(e)            
            raise GpioDriverOfflineError(self)

    async def write(self, pin: str, state: bool) -> None:
        _ = pin
        if state:
            onoff = f"F{int(pin)-1}=1"
        else: 
            onoff = f"F{int(pin)-1}=0"
        try:
            async with aiohttp.ClientSession() as session:
                url = f'http://{self.__host}:{self.__port}/ctrl.htm'
                headers={'Content-Type':'text/plain'}
                auth=aiohttp.BasicAuth(self.__user,self.__password)
                async with session.post(url,auth=auth,headers=headers,data=onoff) as resp:
                    await resp.text()
                    if 200 != resp.status:
                        raise GpioDriverOfflineError(self)                        
        
        except Exception as e: 
            get_logger(0).error(e)
            raise GpioDriverOfflineError(self)

    def __str__(self) -> str:
        return f"ANELPWR({self._instance_name})"

    __repr__ = __str__
