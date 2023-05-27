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


import socket
import functools

from typing import Callable
from typing import Any

from ...logging import get_logger

from ... import aiotools

from ...yamlconf import Option

from ...validators.net import valid_ip
from ...validators.net import valid_port
from ...validators.net import valid_mac

from . import GpioDriverOfflineError
from . import BaseUserGpioDriver


# =====
class Plugin(BaseUserGpioDriver):  # pylint: disable=too-many-instance-attributes
    def __init__(  # pylint: disable=super-init-not-called
        self,
        instance_name: str,
        notifier: aiotools.AioNotifier,

        ip: str,
        port: int,
        mac: str,
    ) -> None:

        super().__init__(instance_name, notifier)

        self.__ip = ip
        self.__port = port
        self.__mac = mac

    @classmethod
    def get_plugin_options(cls) -> dict:
        return {
            "ip":   Option("255.255.255.255", type=functools.partial(valid_ip, v6=False)),
            "port": Option(9,  type=valid_port),
            "mac":  Option("", type=valid_mac, if_empty=""),
        }

    @classmethod
    def get_pin_validator(cls) -> Callable[[Any], Any]:
        return str

    async def read(self, pin: str) -> bool:
        _ = pin
        return False

    async def write(self, pin: str, state: bool) -> None:
        _ = pin
        if not state:
            return

        sock: (socket.socket | None) = None
        try:
            # TODO: IPv6 support: http://lists.cluenet.de/pipermail/ipv6-ops/2014-September/010139.html
            sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
            sock.setsockopt(socket.SOL_SOCKET, socket.SO_BROADCAST, 1)
            sock.connect((self.__ip, self.__port))
            sock.send(bytes.fromhex("FF" * 6 + self.__mac.replace(":", "") * 16))
        except Exception:
            get_logger(0).exception("Can't send Wake-on-LAN packet via %s to %s", self, self.__mac)
            raise GpioDriverOfflineError(self)
        finally:
            if sock:
                try:
                    sock.close()
                except Exception:
                    pass

    def __str__(self) -> str:
        return f"WakeOnLan({self._instance_name})"

    __repr__ = __str__
