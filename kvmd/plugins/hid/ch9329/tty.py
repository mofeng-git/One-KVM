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


import os
import serial


class TTY:
    def __init__(self, device_path: str, speed: int, read_timeout: float) -> None:
        self.__tty = serial.Serial(device_path, speed, timeout=read_timeout)
        self.__device_path = device_path

    def has_device(self) -> bool:
        return os.path.exists(self.__device_path)

    def send(self, cmd: list[int]) -> list[int]:
        cmd = self.__wrap_cmd(cmd)
        self.__tty.write(serial.to_bytes(cmd))
        data = list(self.__tty.read(5))
        if data and data[4]:
            more_data = list(self.__tty.read(data[4] + 1))
            data.extend(more_data)
        return data

    def check_res(self, res: list[int]) -> bool:
        res_sum = res.pop()
        return (self.__checksum(res) == res_sum)

    def __wrap_cmd(self, cmd: list[int]) -> list[int]:
        cmd.insert(0, 0xAB)
        cmd.insert(0, 0x57)
        cmd.append(self.__checksum(cmd))
        return cmd

    def __checksum(self, cmd: list[int]) -> int:
        return sum(cmd) % 256


def get_info() -> list[int]:
    return [0x00, 0x01, 0x00]

# RESET = [0x00,0x0F,0x00]
# GET_INFO = [0x00,0x01,0x00]
