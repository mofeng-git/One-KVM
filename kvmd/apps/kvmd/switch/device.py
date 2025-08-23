# ========================================================================== #
#                                                                            #
#    KVMD - The main PiKVM daemon.                                           #
#                                                                            #
#    Copyright (C) 2018-2024  Maxim Devaev <mdevaev@gmail.com>               #
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
import random
import types

import serial

from .lib import tools

from .types import Edid
from .types import Colors

from .proto import Packable
from .proto import Request
from .proto import Response
from .proto import Header

from .proto import BodySwitch
from .proto import BodySetBeacon
from .proto import BodyAtxClick
from .proto import BodySetEdid
from .proto import BodyClearEdid
from .proto import BodySetDummy
from .proto import BodySetColors
from .proto import BodySetQuirks


# =====
class DeviceError(Exception):
    def __init__(self, ex: Exception):
        super().__init__(tools.efmt(ex))


class Device:
    __SPEED = 115200
    __TIMEOUT = 5.0

    def __init__(self, device_path: str) -> None:
        self.__device_path = device_path
        self.__rid = random.randint(1, 0xFFFF)
        self.__tty: (serial.Serial | None) = None
        self.__buf: bytes = b""

    def __enter__(self) -> "Device":
        try:
            self.__tty = serial.Serial(
                self.__device_path,
                baudrate=self.__SPEED,
                timeout=self.__TIMEOUT,
            )
        except Exception as ex:
            raise DeviceError(ex)
        return self

    def __exit__(
        self,
        _exc_type: type[BaseException],
        _exc: BaseException,
        _tb: types.TracebackType,
    ) -> None:

        if self.__tty is not None:
            try:
                self.__tty.close()
            except Exception:
                pass
            self.__tty = None

    def has_device(self) -> bool:
        return os.path.exists(self.__device_path)

    def get_fd(self) -> int:
        assert self.__tty is not None
        return self.__tty.fd

    def read_all(self) -> list[Response]:
        assert self.__tty is not None
        try:
            if not self.__tty.in_waiting:
                return []
            self.__buf += self.__tty.read_all()
        except Exception as ex:
            raise DeviceError(ex)

        results: list[Response] = []
        while True:
            try:
                begin = self.__buf.index(0xF1)
            except ValueError:
                break
            try:
                end = self.__buf.index(0xF2, begin)
            except ValueError:
                break
            msg = self.__buf[begin + 1:end]
            if 0xF1 in msg:
                # raise RuntimeError(f"Found 0xF1 inside the message: {msg!r}")
                break
            self.__buf = self.__buf[end + 1:]
            msg = self.__unescape(msg)
            resp = Response.unpack(msg)
            if resp is not None:
                results.append(resp)
        return results

    def __unescape(self, msg: bytes) -> bytes:
        if 0xF0 not in msg:
            return msg
        unesc: list[int] = []
        esc = False
        for ch in msg:
            if ch == 0xF0:
                esc = True
            else:
                if esc:
                    ch ^= 0xFF
                    esc = False
                unesc.append(ch)
        return bytes(unesc)

    def request_reboot(self, unit: int, bootloader: bool) -> int:
        return self.__send_request((Header.BOOTLOADER if bootloader else Header.REBOOT), unit, None)

    def request_state(self) -> int:
        return self.__send_request(Header.STATE, 0xFF, None)

    def request_switch(self, unit: int, ch: int) -> int:
        return self.__send_request(Header.SWITCH, unit, BodySwitch(ch))

    def request_beacon(self, unit: int, ch: int, on: bool) -> int:
        return self.__send_request(Header.BEACON, unit, BodySetBeacon(ch, on))

    def request_atx_leds(self) -> int:
        return self.__send_request(Header.ATX_LEDS, 0xFF, None)

    def request_atx_cp(self, unit: int, ch: int, delay_ms: int) -> int:
        return self.__send_request(Header.ATX_CLICK, unit, BodyAtxClick(ch, BodyAtxClick.POWER, delay_ms))

    def request_atx_cr(self, unit: int, ch: int, delay_ms: int) -> int:
        return self.__send_request(Header.ATX_CLICK, unit, BodyAtxClick(ch, BodyAtxClick.RESET, delay_ms))

    def request_set_edid(self, unit: int, ch: int, edid: Edid) -> int:
        if edid.valid:
            return self.__send_request(Header.SET_EDID, unit, BodySetEdid(ch, edid))
        return self.__send_request(Header.CLEAR_EDID, unit, BodyClearEdid(ch))

    def request_set_dummy(self, unit: int, ch: int, on: bool) -> int:
        return self.__send_request(Header.SET_DUMMY, unit, BodySetDummy(ch, on))

    def request_set_colors(self, unit: int, ch: int, colors: Colors) -> int:
        return self.__send_request(Header.SET_COLORS, unit, BodySetColors(ch, colors))

    def request_set_quirks(self, unit: int, ignore_hpd: bool) -> int:
        return self.__send_request(Header.SET_QUIRKS, unit, BodySetQuirks(ignore_hpd))

    def __send_request(self, op: int, unit: int, body: (Packable | None)) -> int:
        assert self.__tty is not None
        req = Request(Header(
            proto=1,
            rid=self.__get_next_rid(),
            op=op,
            unit=unit,
        ), body)
        data: list[int] = [0xF1]
        for ch in req.pack():
            if 0xF0 <= ch <= 0xF2:
                data.append(0xF0)
                ch ^= 0xFF
            data.append(ch)
        data.append(0xF2)
        try:
            self.__tty.write(bytes(data))
            self.__tty.flush()
        except Exception as ex:
            raise DeviceError(ex)
        return req.header.rid

    def __get_next_rid(self) -> int:
        rid = self.__rid
        self.__rid += 1
        if self.__rid > 0xFFFF:
            self.__rid = 1
        return rid
