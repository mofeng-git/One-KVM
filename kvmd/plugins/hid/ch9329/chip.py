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


import serial
import contextlib

from typing import Generator


# =====
class ChipResponseError(Exception):
    pass


# =====
class ChipConnection:
    def __init__(self, tty: serial.Serial) -> None:
        self.__tty = tty

    def xfer(self, cmd: bytes) -> int:
        self.__send(cmd)
        return self.__recv()

    def __send(self, cmd: bytes) -> None:
        # RESET = [0x00,0x0F,0x00]
        # GET_INFO = [0x00,0x01,0x00]
        if len(cmd) == 0:
            cmd = b"\x00\x01\x00"
        cmd = b"\x57\xAB" + cmd
        cmd += self.__make_checksum(cmd).to_bytes(1, "big")
        self.__tty.write(cmd)

    def __recv(self) -> int:
        data = self.__tty.read(5)
        if len(data) < 5:
            raise ChipResponseError("Too short response, HID might be disconnected")

        if data and data[4]:
            data += self.__tty.read(data[4] + 1)

        if self.__make_checksum(data[:-1]) != data[-1]:
            raise ChipResponseError("Invalid response checksum")

        if data[4] == 1 and data[5] != 0:
            raise ChipResponseError(f"Response error code = {data[5]!r}")

        # led_byte (info) response
        return (data[7] if data[3] == 0x81 else -1)

    def __make_checksum(self, cmd: bytes) -> int:
        return (sum(cmd) % 256)


class Chip:
    def __init__(self, device_path: str, speed: int, read_timeout: float) -> None:
        self.__device_path = device_path
        self.__speed = speed
        self.__read_timeout = read_timeout

    @contextlib.contextmanager
    def connected(self) -> Generator[ChipConnection, None, None]:  # type: ignore
        with serial.Serial(self.__device_path, self.__speed, timeout=self.__read_timeout) as tty:
            yield ChipConnection(tty)
