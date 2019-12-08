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


import socket

from typing import Dict
from typing import Optional

from ...logging import get_logger

from ...errors import OperationError

from ... import aiotools


# =====
class WolDisabledError(OperationError):
    def __init__(self) -> None:
        super().__init__("WoL is disabled")


# =====
class WakeOnLan:
    def __init__(self, ip: str, port: int, mac: str) -> None:
        self.__ip = ip
        self.__port = port
        self.__mac = mac
        self.__magic = b""

        if mac:
            assert len(mac) == 17, mac
            self.__magic = bytes.fromhex("FF" * 6 + mac.replace(":", "") * 16)

    def get_state(self) -> Dict:
        return {
            "enabled": bool(self.__magic),
            "target": {
                "ip": self.__ip,
                "port": self.__port,
                "mac": self.__mac,
            },
        }

    @aiotools.atomic
    async def wakeup(self) -> None:
        if not self.__magic:
            raise WolDisabledError()
        await self.__inner_wakeup()

    @aiotools.tasked
    @aiotools.muted("Can't perform Wake-on-LAN or operation was not completed")
    async def __inner_wakeup(self) -> None:
        logger = get_logger(0)
        logger.info("Waking up %s (%s:%s) using Wake-on-LAN ...", self.__mac, self.__ip, self.__port)
        sock: Optional[socket.socket] = None
        try:
            # TODO: IPv6 support: http://lists.cluenet.de/pipermail/ipv6-ops/2014-September/010139.html
            sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
            sock.setsockopt(socket.SOL_SOCKET, socket.SO_BROADCAST, 1)
            sock.connect((self.__ip, self.__port))
            sock.send(self.__magic)
        except Exception:
            logger.exception("Can't send Wake-on-LAN packet")
        else:
            logger.info("Wake-on-LAN packet sent")
        finally:
            if sock:
                try:
                    sock.close()
                except Exception:
                    pass
