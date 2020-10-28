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


import os
import contextlib

from typing import Dict
from typing import Generator
from typing import Any

import serial

from ...yamlconf import Option

from ...validators.basic import valid_float_f01
from ...validators.os import valid_abs_path
from ...validators.hw import valid_tty_speed

from ._mcu import BasePhyConnection
from ._mcu import BasePhy
from ._mcu import BaseMcuHid


# =====
class _SerialPhyConnection(BasePhyConnection):
    def __init__(self, tty: serial.Serial) -> None:
        self.__tty = tty

    def send(self, request: bytes, receive: int) -> bytes:
        if self.__tty.in_waiting:
            self.__tty.read_all()
        assert self.__tty.write(request) == len(request)
        return self.__tty.read(receive)


class _SerialPhy(BasePhy):
    def __init__(
        self,
        device_path: str,
        speed: int,
        read_timeout: float,
    ) -> None:

        self.__device_path = device_path
        self.__speed = speed
        self.__read_timeout = read_timeout

    def has_device(self) -> bool:
        return os.path.exists(self.__device_path)

    @contextlib.contextmanager
    def connected(self) -> Generator[_SerialPhyConnection, None, None]:  # type: ignore
        with serial.Serial(self.__device_path, self.__speed, timeout=self.__read_timeout) as tty:
            yield _SerialPhyConnection(tty)


# =====
class Plugin(BaseMcuHid):
    def __init__(
        self,
        device_path: str,
        speed: int,
        read_timeout: float,
        **kwargs: Any,
    ) -> None:

        super().__init__(
            phy=_SerialPhy(device_path, speed, read_timeout),
            **kwargs,
        )

    @classmethod
    def get_plugin_options(cls) -> Dict:
        return {
            "device":       Option("",     type=valid_abs_path, unpack_as="device_path"),
            "speed":        Option(115200, type=valid_tty_speed),
            "read_timeout": Option(2.0,    type=valid_float_f01),
            **BaseMcuHid.get_plugin_options(),
        }
