# ========================================================================== #
#                                                                            #
#    KVMD - The main Pi-KVM daemon.                                          #
#                                                                            #
#    Copyright (C) 2020  Maxim Devaev <mdevaev@gmail.com>                    #
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


import asyncio

from typing import Dict
from typing import Coroutine

from ....logging import get_logger

from .... import aiotools

from .errors import RfbError
from .errors import RfbConnectionError

from .encodings import RfbEncodings
from .encodings import RfbClientEncodings

from .stream import RfbClientStream


# =====
class RfbClient(RfbClientStream):
    # https://github.com/rfbproto/rfbproto/blob/master/rfbproto.rst
    # https://www.toptal.com/java/implementing-remote-framebuffer-server-java
    # https://github.com/TigerVNC/tigervnc

    def __init__(
        self,
        reader: asyncio.StreamReader,
        writer: asyncio.StreamWriter,

        width: int,
        height: int,
        name: str,
    ) -> None:

        super().__init__(reader, writer)

        self._width = width
        self._height = height
        self._name = name

        self._encodings = RfbClientEncodings(frozenset())

        self._lock = asyncio.Lock()

        get_logger(0).info("Connected client: %s", self._remote)

    # =====

    async def _run(self, **coros: Coroutine) -> None:
        tasks = list(map(asyncio.create_task, [
            self.__wrapper(name, coro)
            for (name, coro) in {"main": self.__main_task_loop(), **coros}.items()
        ]))
        try:
            await aiotools.wait_first(*tasks)
        finally:
            for task in tasks:
                task.cancel()

    async def __wrapper(self, name: str, coro: Coroutine) -> None:
        logger = get_logger(0)
        try:
            await coro
            raise RuntimeError("Subtask just finished without any exception")
        except asyncio.CancelledError:
            logger.info("[%s] Client %s: Cancelling ...", name, self._remote)
            raise
        except RfbConnectionError as err:
            logger.info("[%s] Client %s: Gone (%s): Disconnected", name, self._remote, str(err))
        except RfbError as err:
            logger.info("[%s] Client %s: %s: Disconnected", name, self._remote, str(err))
        except Exception:
            logger.exception("[%s] Unhandled exception with client %s: Disconnected", name, self._remote)

    async def __main_task_loop(self) -> None:
        try:
            rfb_version = await self.__handshake_version()
            await self.__handshake_security(rfb_version)
            await self.__handshake_init()
            await self.__main_loop()
        finally:
            self._close()

    # =====

    async def _authorize(self, user: str, passwd: str) -> bool:
        raise NotImplementedError

    async def _on_key_event(self, code: int, state: bool) -> None:
        raise NotImplementedError

    async def _on_pointer_event(self, buttons: Dict[str, bool], wheel: Dict[str, int], move: Dict[str, int]) -> None:
        raise NotImplementedError

    async def _on_cut_event(self, text: str) -> None:
        raise NotImplementedError

    async def _on_set_encodings(self) -> None:
        raise NotImplementedError

    async def _on_fb_update_request(self) -> None:
        raise NotImplementedError

    # =====

    async def _send_fb(self, jpeg: bytes) -> None:
        assert self._encodings.has_tight
        assert self._encodings.tight_jpeg_quality > 0
        assert len(jpeg) <= 4194303, len(jpeg)
        await self._write_fb_update(self._width, self._height, RfbEncodings.TIGHT, drain=False)
        length = len(jpeg)
        if length <= 127:
            await self._write_struct("", bytes([0b10011111, length & 0x7F]), jpeg)
        elif length <= 16383:
            await self._write_struct("", bytes([0b10011111, length & 0x7F | 0x80, length >> 7 & 0x7F]), jpeg)
        else:
            await self._write_struct("", bytes([0b10011111, length & 0x7F | 0x80, length >> 7 & 0x7F | 0x80, length >> 14 & 0xFF]), jpeg)

    async def _send_resize(self, width: int, height: int) -> None:
        assert self._encodings.has_resize
        await self._write_fb_update(width, height, RfbEncodings.RESIZE)
        self._width = width
        self._height = height

    async def _send_rename(self, name: str) -> None:
        assert self._encodings.has_rename
        await self._write_fb_update(0, 0, RfbEncodings.RENAME, drain=False)
        await self._write_reason(name)
        self._name = name

    async def _send_leds_state(self, caps: bool, scroll: bool, num: bool) -> None:
        assert self._encodings.has_leds_state
        await self._write_fb_update(0, 0, RfbEncodings.LEDS_STATE, drain=False)
        await self._write_struct("B", 0x1 & scroll | 0x2 & num | 0x4 & caps)

    # =====

    async def __handshake_version(self) -> int:
        # The only published protocol versions at this time are 3.3, 3.7, 3.8.
        # Version 3.5 was wrongly reported by some clients, but it should be
        # interpreted by all servers as 3.3

        await self._write_struct("", b"RFB 003.008\n")

        response = await self._read_text(12)
        if (
            not response.startswith("RFB 003.00")
            or not response.endswith("\n")
            or response[-2] not in ["3", "5", "7", "8"]
        ):
            raise RfbError(f"Invalid version response: {response!r}")

        try:
            version = int(response[-2])
        except ValueError:
            raise RfbError(f"Invalid version response: {response!r}")
        return (3 if version == 5 else version)

    # =====

    async def __handshake_security(self, rfb_version: int) -> None:
        if rfb_version == 3:
            await self.__handshake_security_v3(rfb_version)
        else:
            await self.__handshake_security_v7_plus(rfb_version)

    async def __handshake_security_v3(self, rfb_version: int) -> None:
        assert rfb_version == 3

        await self._write_struct("L", 0, drain=False)  # Refuse old clients using the invalid security type
        msg = "The client uses a very old protocol 3.3; required 3.7 at least"
        await self._write_reason(msg)
        raise RfbError(msg)

    async def __handshake_security_v7_plus(self, rfb_version: int) -> None:
        assert rfb_version >= 7

        vencrypt = 19
        await self._write_struct("B B", 1, vencrypt)  # One security type, VeNCrypt

        security_type = await self._read_number("B")
        if security_type != vencrypt:
            raise RfbError(f"Invalid security type: {security_type}; expected VeNCrypt({vencrypt})")

        # -----

        await self._write_struct("BB", 0, 2)  # VeNCrypt 0.2

        vencrypt_version = "%d.%d" % (await self._read_struct("BB"))
        if vencrypt_version != "0.2":
            await self._write_struct("B", 1)  # Unsupported
            raise RfbError(f"Unsupported VeNCrypt version: {vencrypt_version}")

        await self._write_struct("B", 0)

        # -----

        plain = 256
        await self._write_struct("B L", 1, plain)  # One auth subtype, plain

        auth_type = await self._read_number("L")
        if auth_type != plain:
            raise RfbError(f"Invalid auth type: {auth_type}; expected Plain({plain})")

        # -----

        (user_length, passwd_length) = await self._read_struct("LL")
        user = await self._read_text(user_length)
        passwd = await self._read_text(passwd_length)

        if (await self._authorize(user, passwd)):
            get_logger(0).info("[main] Client %s: Access granted for user %r", self._remote, user)
            await self._write_struct("L", 0)
        else:
            await self._write_struct("L", 1, drain=(rfb_version < 8))
            if rfb_version >= 8:
                await self._write_reason("Invalid username or password")
            raise RfbError(f"Access denied for user {user!r}")

    # =====

    async def __handshake_init(self) -> None:
        await self._read_number("B")  # Shared flag, ignored

        await self._write_struct("HH", self._width, self._height, drain=False)
        await self._write_struct(
            "BB?? HHH BBB xxx",
            32,     # Bits per pixel
            24,     # Depth
            False,  # Big endian
            True,   # True color
            255,    # Red max
            255,    # Green max
            255,    # Blue max
            16,     # Red shift
            8,      # Green shift
            0,      # Blue shift
            drain=False,
        )
        await self._write_reason(self._name)

    # =====

    async def __main_loop(self) -> None:
        handlers = {
            0: self.__handle_set_pixel_format,
            2: self.__handle_set_encodings,
            3: self.__handle_fb_update_request,
            4: self.__handle_key_event,
            5: self.__handle_pointer_event,
            6: self.__handle_client_cut_text,
        }
        while True:
            msg_type = await self._read_number("B")
            if (handler := handlers.get(msg_type)) is not None:  # noqa: E203,E231
                await handler()  # type: ignore  # mypy bug
            else:
                raise RfbError(f"Unknown message type: {msg_type}")

    async def __handle_set_pixel_format(self) -> None:
        # JpegCompression may only be used when bits-per-pixel is either 16 or 32
        bits_per_pixel = (await self._read_struct("xxx BB?? HHH BBB xxx"))[0]
        if bits_per_pixel not in [16, 32]:
            raise RfbError(f"Requested unsupported {bits_per_pixel=} for Tight JPEG; required 16 or 32")

    async def __handle_set_encodings(self) -> None:
        encodings_count = (await self._read_struct("x H"))[0]
        if encodings_count > 1024:
            raise RfbError(f"Too many encodings: {encodings_count}")
        self._encodings = RfbClientEncodings(frozenset(await self._read_struct("l" * encodings_count)))
        get_logger(0).info("[main] Client %s: Features: resize=%d; rename=%d; leds=%d",
                           self._remote, self._encodings.has_resize, self._encodings.has_rename, self._encodings.has_leds_state)
        self.__check_tight_jpeg()
        await self._on_set_encodings()

    async def __handle_fb_update_request(self) -> None:
        self.__check_tight_jpeg()  # If we don't receive SetEncodings from client
        await self._read_struct("? HH HH")  # Ignore any arguments, just perform the full update
        await self._on_fb_update_request()

    async def __handle_key_event(self) -> None:
        (state, code) = await self._read_struct("? xx L")
        await self._on_key_event(code, state)  # type: ignore

    async def __handle_pointer_event(self) -> None:
        (buttons, to_x, to_y) = await self._read_struct("B HH")
        await self._on_pointer_event(
            buttons={
                "left": bool(buttons & 0x1),
                "right": bool(buttons & 0x4),
                "middle": bool(buttons & 0x2),
            },
            wheel={
                "x": (-4 if buttons & 0x40 else (4 if buttons & 0x20 else 0)),
                "y": (-4 if buttons & 0x10 else (4 if buttons & 0x8 else 0)),
            },
            move={
                "x": round(to_x / self._width * 65535 + -32768),
                "y": round(to_y / self._height * 65535 + -32768),
            },
        )

    async def __handle_client_cut_text(self) -> None:
        length = (await self._read_struct("xxx L"))[0]
        text = await self._read_text(length)
        await self._on_cut_event(text)

    def __check_tight_jpeg(self) -> None:
        # JpegCompression may only be used when the client has advertized
        # a quality level using the JPEG Quality Level Pseudo-encoding
        if not self._encodings.has_tight or self._encodings.tight_jpeg_quality == 0:
            raise RfbError(f"Tight JPEG encoding is not supported by client: {self._encodings}")
