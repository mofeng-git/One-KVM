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
import time

from typing import List
from typing import Dict
from typing import Generator
from typing import Callable
from typing import Any

import spidev

from ...logging import get_logger

from ...yamlconf import Option

from ...validators.basic import valid_int_f0
from ...validators.basic import valid_int_f1
from ...validators.basic import valid_float_f0
from ...validators.basic import valid_float_f01

from ._mcu import BasePhyConnection
from ._mcu import BasePhy
from ._mcu import BaseMcuHid


# =====
class _SpiPhyConnection(BasePhyConnection):
    def __init__(
        self,
        xfer: Callable[[bytes], bytes],
        read_timeout: float,
        read_delay: float,
    ) -> None:

        self.__xfer = xfer
        self.__read_timeout = read_timeout
        self.__read_delay = read_delay

    def send(self, request: bytes) -> bytes:
        assert len(request) == 8
        self.__xfer(request)

        response: List[int] = []
        deadline_ts = time.time() + self.__read_timeout
        found = False
        while time.time() < deadline_ts:
            if not found:
                time.sleep(self.__read_delay)
            for byte in self.__xfer(b"\x00" * (4 - len(response))):
                if not found:
                    if byte == 0:
                        continue
                    found = True
                response.append(byte)
                if len(response) == 4:
                    break
            if len(response) == 4:
                break
        else:
            get_logger(0).error("SPI timeout reached while responce waiting")
            return b""
        return bytes(response)


class _SpiPhy(BasePhy):
    def __init__(
        self,
        bus: int,
        chip: int,
        max_freq: int,
        block_usec: int,
        read_timeout: float,
        read_delay: float,
    ) -> None:

        self.__bus = bus
        self.__chip = chip
        self.__max_freq = max_freq
        self.__block_usec = block_usec
        self.__read_timeout = read_timeout
        self.__read_delay = read_delay

    def has_device(self) -> bool:
        return os.path.exists(f"/dev/spidev{self.__bus}.{self.__chip}")

    @contextlib.contextmanager
    def connected(self) -> Generator[_SpiPhyConnection, None, None]:  # type: ignore
        with contextlib.closing(spidev.SpiDev(self.__bus, self.__chip)) as spi:
            spi.mode = 0
            spi.max_speed_hz = self.__max_freq

            def xfer(data: bytes) -> bytes:
                return spi.xfer(data, self.__max_freq, self.__block_usec)

            yield _SpiPhyConnection(
                xfer=xfer,
                read_timeout=self.__read_timeout,
                read_delay=self.__read_delay,
            )


# =====
class Plugin(BaseMcuHid):
    def __init__(
        self,
        bus: int,
        chip: int,
        max_freq: int,
        block_usec: int,
        read_timeout: float,
        read_delay: float,
        **kwargs: Any,
    ) -> None:

        super().__init__(
            phy=_SpiPhy(bus, chip, max_freq, block_usec, read_timeout, read_delay),
            **kwargs,
        )

    @classmethod
    def get_plugin_options(cls) -> Dict:
        return {
            "bus":          Option(0,      type=valid_int_f0),
            "chip":         Option(0,      type=valid_int_f0),
            "max_freq":     Option(400000, type=valid_int_f1),
            "block_usec":   Option(1,      type=valid_int_f0),
            "read_timeout": Option(2.0,    type=valid_float_f01),
            "read_delay":   Option(0.001,  type=valid_float_f0),
            **BaseMcuHid.get_plugin_options(),
        }
