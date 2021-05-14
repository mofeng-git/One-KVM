# ========================================================================== #
#                                                                            #
#    KVMD - The main Pi-KVM daemon.                                          #
#                                                                            #
#    Copyright (C) 2018-2021  Maxim Devaev <mdevaev@gmail.com>               #
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


import re
import functools
import errno
import time

from typing import Tuple
from typing import Dict
from typing import Optional

import serial
import socket
import binascii

from ...logging import get_logger

from ... import aiotools
from ... import aiomulti
from ... import aioproc

from ...yamlconf import Option

from ...validators.basic import valid_number
from ...validators.basic import valid_float_f01
from ...validators.os import valid_abs_path
from ...validators.hw import valid_tty_speed
from ...validators.net import valid_ip_or_host
from ...validators.net import valid_port

from . import BaseUserGpioDriver


# =====
class Plugin(BaseUserGpioDriver):  # pylint: disable=too-many-instance-attributes
    def __init__(
        self,
        instance_name: str,
        notifier: aiotools.AioNotifier,

        tesmart_host: str,
        tesmart_port: int,
        max_ports: int,

    ) -> None:

        super().__init__(instance_name, notifier)

        self.__tesmart_host = tesmart_host
        self.__tesmart_port = tesmart_port
        self.__max_ports = max_ports
        self.__switch_state: Dict[int, bool] = {}
        self.__tes_socket: socket

    @classmethod
    def get_plugin_options(cls) -> Dict:
        return {
            "tesmart_host":       Option("192.168.1.10", type=valid_ip_or_host),
            "tesmart_port":        Option(5000, type=valid_port),
            "max_ports":            Option(8,      type=functools.partial(valid_number, min=4, max=16)),
        }

    def register_input(self, pin: int, debounce: float) -> None:
        _ = pin
        _ = debounce

    def register_output(self, port: int, initial: Optional[bool]) -> None:
        if port <= self.__max_ports:
            self.__switch_state[port] = initial

    def prepare(self) -> None:
        self.__tes_socket = socket.create_connection((self.__tesmart_host,self.__tesmart_port))
        self.__update_state()

    def __update_state(self) -> None:
        for port in self.__switch_state:
            self.__switch_state[port] = False
        selport = self.__get_selected_port()
        if selport in self.__switch_state:
            self.__switch_state[selport] = True

    def __get_selected_port(self) -> int:
        retint = self.__send_tesmart_command("1000")
        return retint+1

    def __send_tesmart_command(self,tes_cmd: str) -> int:
        full_cmd="AABB03"+tes_cmd+"EE"
        binstr = binascii.unhexlify(full_cmd)
        self.__tes_socket.sendall(binstr)
        retstr=self.__tes_socket.recv(6)
        return int(bytearray(retstr)[4])

    async def run(self) -> None:
        pass

    def cleanup(self) -> None:
        pass

    async def read(self, pin: int) -> bool:
        if pin in self.__switch_state:
            return self.__switch_state[pin]
        return False

    async def write(self, pin: int, state: bool) -> None:
        if state == False:
            return
        part_cmd="01"+format(pin,"#04x")[2:4]
        writeret = self.__send_tesmart_command(part_cmd)
        self.__update_state()

    # =====

    def __str__(self) -> str:
        return f"tesmart({self._instance_name})"

    __repr__ = __str__
