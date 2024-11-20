# ========================================================================== #
#                                                                            #
#    KVMD - The main PiKVM daemon.                                           #
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


import os
import asyncio
import socket
import dataclasses
import contextlib

import aiohttp

from ...logging import get_logger

from ...keyboard.keysym import SymmapModifiers
from ...keyboard.keysym import build_symmap
from ...keyboard.mappings import WebModifiers
from ...keyboard.mappings import X11Modifiers
from ...keyboard.mappings import AT1_TO_WEB

from ...clients.kvmd import KvmdClientWs
from ...clients.kvmd import KvmdClientSession
from ...clients.kvmd import KvmdClient

from ...clients.streamer import StreamerError
from ...clients.streamer import StreamerPermError
from ...clients.streamer import StreamFormats
from ...clients.streamer import BaseStreamerClient

from ... import tools
from ... import aiotools
from ... import network

from .rfb import RfbClient
from .rfb.stream import rfb_format_remote
from .rfb.errors import RfbError

from .vncauth import VncAuthKvmdCredentials
from .vncauth import VncAuthManager

from .render import make_text_jpeg


# =====
@dataclasses.dataclass()
class _SharedParams:
    width: int = dataclasses.field(default=800)
    height: int = dataclasses.field(default=600)
    name: str = dataclasses.field(default="PiKVM")


class _Client(RfbClient):  # pylint: disable=too-many-instance-attributes
    def __init__(  # pylint: disable=too-many-arguments,too-many-locals
        self,
        reader: asyncio.StreamReader,
        writer: asyncio.StreamWriter,
        tls_ciphers: str,
        tls_timeout: float,
        x509_cert_path: str,
        x509_key_path: str,

        desired_fps: int,
        mouse_output: str,
        keymap_name: str,
        symmap: dict[int, dict[int, str]],

        kvmd: KvmdClient,
        streamers: list[BaseStreamerClient],

        vnc_credentials: dict[str, VncAuthKvmdCredentials],
        vencrypt: bool,
        none_auth_only: bool,
        shared_params: _SharedParams,
    ) -> None:

        self.__vnc_credentials = vnc_credentials

        super().__init__(
            reader=reader,
            writer=writer,
            tls_ciphers=tls_ciphers,
            tls_timeout=tls_timeout,
            x509_cert_path=x509_cert_path,
            x509_key_path=x509_key_path,
            vnc_passwds=list(vnc_credentials),
            vencrypt=vencrypt,
            none_auth_only=none_auth_only,
            **dataclasses.asdict(shared_params),
        )

        self.__desired_fps = desired_fps
        self.__mouse_output = mouse_output
        self.__keymap_name = keymap_name
        self.__symmap = symmap

        self.__kvmd = kvmd
        self.__streamers = streamers

        self.__shared_params = shared_params

        self.__stage1_authorized = aiotools.AioStage()
        self.__stage2_encodings_accepted = aiotools.AioStage()
        self.__stage3_ws_connected = aiotools.AioStage()

        self.__kvmd_session: (KvmdClientSession | None) = None
        self.__kvmd_ws: (KvmdClientWs | None) = None

        self.__fb_queue: "asyncio.Queue[dict]" = asyncio.Queue()
        self.__fb_has_key = False

        # Эти состояния шарить не обязательно - бекенд исключает дублирующиеся события.
        # Все это нужно только чтобы не посылать лишние жсоны в сокет KVMD
        self.__mouse_buttons: dict[str, (bool | None)] = dict.fromkeys(["left", "right", "middle"], None)
        self.__mouse_move = {"x": -1, "y": -1}

        self.__modifiers = 0

    # =====

    async def run(self) -> None:
        try:
            await self._run(
                kvmd=self.__kvmd_task_loop(),
                streamer=self.__streamer_task_loop(),
                fb_sender=self.__fb_sender_task_loop(),
            )
        finally:
            await aiotools.shield_fg(self.__cleanup())

    async def __cleanup(self) -> None:
        if self.__kvmd_session:
            await self.__kvmd_session.close()
            self.__kvmd_session = None

    # =====

    async def __kvmd_task_loop(self) -> None:
        logger = get_logger(0)
        await self.__stage1_authorized.wait_passed()

        logger.info("%s [kvmd]: Waiting for the SetEncodings message ...", self._remote)
        if not (await self.__stage2_encodings_accepted.wait_passed(timeout=5)):
            raise RfbError("No SetEncodings message recieved from the client in 5 secs")

        assert self.__kvmd_session
        try:
            logger.info("%s [kvmd]: Applying HID params: mouse_output=%s ...", self._remote, self.__mouse_output)
            await self.__kvmd_session.hid.set_params(mouse_output=self.__mouse_output)

            async with self.__kvmd_session.ws() as self.__kvmd_ws:
                logger.info("%s [kvmd]: Connected to KVMD websocket", self._remote)
                self.__stage3_ws_connected.set_passed()
                async for (event_type, event) in self.__kvmd_ws.communicate():
                    await self.__process_ws_event(event_type, event)
                raise RfbError("KVMD closed the websocket (the server may have been stopped)")
        finally:
            self.__kvmd_ws = None

    async def __process_ws_event(self, event_type: str, event: dict) -> None:
        if event_type == "info_meta_state":
            try:
                host = event["server"]["host"]
            except Exception:
                host = None
            else:
                if isinstance(host, str):
                    name = f"PiKVM: {host}"
                    if self._encodings.has_rename:
                        await self._send_rename(name)
                    self.__shared_params.name = name

        elif event_type == "hid_state":
            if self._encodings.has_leds_state:
                await self._send_leds_state(**event["keyboard"]["leds"])

    # =====

    async def __streamer_task_loop(self) -> None:
        logger = get_logger(0)
        await self.__stage3_ws_connected.wait_passed()
        streamer = self.__get_preferred_streamer()
        while True:
            try:
                streaming = False
                async with streamer.reading() as read_frame:
                    while True:
                        frame = await read_frame(not self.__fb_has_key)
                        if not streaming:
                            logger.info("%s [streamer]: Streaming ...", self._remote)
                            streaming = True
                        if frame["online"]:
                            await self.__queue_frame(frame)
                        else:
                            await self.__queue_frame("No signal")
            except StreamerError as err:
                if isinstance(err, StreamerPermError):
                    streamer = self.__get_default_streamer()
                    logger.info("%s [streamer]: Permanent error: %s; switching to %s ...", self._remote, err, streamer)
                else:
                    logger.info("%s [streamer]: Waiting for stream: %s", self._remote, err)
                await self.__queue_frame("Waiting for stream ...")
                await asyncio.sleep(1)

    def __get_preferred_streamer(self) -> BaseStreamerClient:
        formats = {
            StreamFormats.JPEG: "has_tight",
            StreamFormats.H264: "has_h264",
        }
        streamer: (BaseStreamerClient | None) = None
        for streamer in self.__streamers:
            if getattr(self._encodings, formats[streamer.get_format()]):
                get_logger(0).info("%s [streamer]: Using preferred %s", self._remote, streamer)
                return streamer
        raise RuntimeError("No streamers found")

    def __get_default_streamer(self) -> BaseStreamerClient:
        streamer = self.__streamers[-1]
        get_logger(0).info("%s [streamer]: Using default %s", self._remote, streamer)
        return streamer

    async def __queue_frame(self, frame: (dict | str)) -> None:
        if isinstance(frame, str):
            frame = await self.__make_text_frame(frame)
        if self.__fb_queue.qsize() > 10:
            self.__fb_queue.get_nowait()
        self.__fb_queue.put_nowait(frame)

    async def __make_text_frame(self, text: str) -> dict:
        return {
            "data": (await make_text_jpeg(self._width, self._height, self._encodings.tight_jpeg_quality, text)),
            "width": self._width,
            "height": self._height,
            "format": StreamFormats.JPEG,
        }

    async def __fb_sender_task_loop(self) -> None:  # pylint: disable=too-many-branches
        last: (dict | None) = None
        async for _ in self._send_fb_allowed():
            while True:
                frame = await self.__fb_queue.get()
                if (
                    last is None  # pylint: disable=too-many-boolean-expressions
                    or frame["format"] == StreamFormats.JPEG
                    or last["format"] != frame["format"]
                    or (frame["format"] == StreamFormats.H264 and (
                        frame["key"]
                        or last["width"] != frame["width"]
                        or last["height"] != frame["height"]
                        or len(last["data"]) + len(frame["data"]) > 4194304
                    ))
                ):
                    self.__fb_has_key = (frame["format"] == StreamFormats.H264 and frame["key"])
                    last = frame
                    if self.__fb_queue.qsize() == 0:
                        break
                    continue
                assert frame["format"] == StreamFormats.H264
                last["data"] += frame["data"]
                if self.__fb_queue.qsize() == 0:
                    break

            if self._width != last["width"] or self._height != last["height"]:
                self.__shared_params.width = last["width"]
                self.__shared_params.height = last["height"]
                if not self._encodings.has_resize:
                    msg = (
                        f"Resoultion changed: {self._width}x{self._height}"
                        f" -> {last['width']}x{last['height']}\nPlease reconnect"
                    )
                    await self._send_fb_jpeg((await self.__make_text_frame(msg))["data"])
                    continue
                await self._send_resize(last["width"], last["height"])

            if len(last["data"]) == 0:
                # Вдруг какой-то баг
                await self._send_fb_allow_again()
                continue

            if last["format"] == StreamFormats.JPEG:
                await self._send_fb_jpeg(last["data"])
            elif last["format"] == StreamFormats.H264:
                if not self._encodings.has_h264:
                    raise RfbError("The client doesn't want to accept H264 anymore")
                if self.__fb_has_key:
                    await self._send_fb_h264(last["data"])
                else:
                    await self._send_fb_allow_again()
            else:
                raise RuntimeError(f"Unknown format: {last['format']}")
            last["data"] = b""

    # =====

    async def _authorize_userpass(self, user: str, passwd: str) -> bool:
        self.__kvmd_session = self.__kvmd.make_session(user, passwd)
        if (await self.__kvmd_session.auth.check()):
            self.__stage1_authorized.set_passed()
            return True
        return False

    async def _on_authorized_vnc_passwd(self, passwd: str) -> str:
        kc = self.__vnc_credentials[passwd]
        if (await self._authorize_userpass(kc.user, kc.passwd)):
            return kc.user
        return ""

    async def _on_authorized_none(self) -> bool:
        return (await self._authorize_userpass("", ""))

    # =====

    async def _on_key_event(self, code: int, state: bool) -> None:
        is_modifier = self.__switch_modifiers(code, state)
        variants = self.__symmap.get(code)
        fake_shift = False

        if variants:
            if is_modifier:
                web_key = variants.get(0)
            else:
                web_key = variants.get(self.__modifiers)
                if web_key is None:
                    web_key = variants.get(0)

                if web_key is None and self.__modifiers == 0 and SymmapModifiers.SHIFT in variants:
                    # JUMP doesn't send shift events:
                    #   - https://github.com/pikvm/pikvm/issues/820
                    web_key = variants[SymmapModifiers.SHIFT]
                    fake_shift = True

            if web_key and self.__kvmd_ws:
                if fake_shift:
                    await self.__kvmd_ws.send_key_event(WebModifiers.SHIFT_LEFT, True)
                await self.__kvmd_ws.send_key_event(web_key, state)
                if fake_shift:
                    await self.__kvmd_ws.send_key_event(WebModifiers.SHIFT_LEFT, False)

    async def _on_ext_key_event(self, code: int, state: bool) -> None:
        web_key = AT1_TO_WEB.get(code)
        if web_key:
            self.__switch_modifiers(web_key, state)  # Предполагаем, что модификаторы всегда известны
            if self.__kvmd_ws:
                await self.__kvmd_ws.send_key_event(web_key, state)

    def __switch_modifiers(self, key: (int | str), state: bool) -> bool:
        mod = 0
        if key in X11Modifiers.SHIFTS or key in WebModifiers.SHIFTS:
            mod = SymmapModifiers.SHIFT
        elif key == X11Modifiers.ALTGR or key == WebModifiers.ALT_RIGHT:
            mod = SymmapModifiers.ALTGR
        elif key in X11Modifiers.CTRLS or key in WebModifiers.CTRLS:
            mod = SymmapModifiers.CTRL
        if mod == 0:
            return False
        if state:
            self.__modifiers |= mod
        else:
            self.__modifiers &= ~mod
        return True

    async def _on_pointer_event(self, buttons: dict[str, bool], wheel: dict[str, int], move: dict[str, int]) -> None:
        if self.__kvmd_ws:
            if wheel["x"] or wheel["y"]:
                await self.__kvmd_ws.send_mouse_wheel_event(wheel["x"], wheel["y"])

            if self.__mouse_move != move:
                await self.__kvmd_ws.send_mouse_move_event(move["x"], move["y"])
                self.__mouse_move = move

            for (button, state) in buttons.items():
                if self.__mouse_buttons[button] != state:
                    await self.__kvmd_ws.send_mouse_button_event(button, state)
                    self.__mouse_buttons[button] = state

    async def _on_cut_event(self, text: str) -> None:
        assert self.__stage1_authorized.is_passed()
        assert self.__kvmd_session
        logger = get_logger(0)
        logger.info("%s [main]: Printing %d characters ...", self._remote, len(text))
        try:
            (keymap_name, available) = await self.__kvmd_session.hid.get_keymaps()
            if self.__keymap_name in available:
                keymap_name = self.__keymap_name
            await self.__kvmd_session.hid.print(text, 0, keymap_name)
        except Exception:
            logger.exception("%s [main]: Can't print characters", self._remote)

    async def _on_set_encodings(self) -> None:
        assert self.__stage1_authorized.is_passed()
        assert self.__kvmd_session
        self.__stage2_encodings_accepted.set_passed(multi=True)

        has_quality = (await self.__kvmd_session.streamer.get_state())["features"]["quality"]
        quality = (self._encodings.tight_jpeg_quality if has_quality else None)
        get_logger(0).info("%s [main]: Applying streamer params: jpeg_quality=%s; desired_fps=%d ...",
                           self._remote, quality, self.__desired_fps)
        await self.__kvmd_session.streamer.set_params(quality, self.__desired_fps)


# =====
class VncServer:  # pylint: disable=too-many-instance-attributes
    def __init__(  # pylint: disable=too-many-arguments,too-many-locals
        self,
        host: str,
        port: int,
        max_clients: int,

        no_delay: bool,
        keepalive_enabled: bool,
        keepalive_idle: int,
        keepalive_interval: int,
        keepalive_count: int,

        tls_ciphers: str,
        tls_timeout: float,
        x509_cert_path: str,
        x509_key_path: str,

        vencrypt_enabled: bool,

        desired_fps: int,
        mouse_output: str,
        keymap_path: str,

        kvmd: KvmdClient,
        streamers: list[BaseStreamerClient],
        vnc_auth_manager: VncAuthManager,
    ) -> None:

        self.__host = network.get_listen_host(host)
        self.__port = port
        self.__max_clients = max_clients

        keymap_name = os.path.basename(keymap_path)
        symmap = build_symmap(keymap_path)

        self.__vnc_auth_manager = vnc_auth_manager

        shared_params = _SharedParams()

        async def cleanup_client(writer: asyncio.StreamWriter) -> None:
            if (await aiotools.close_writer(writer)):
                get_logger(0).info("%s [entry]: Connection is closed in an emergency", rfb_format_remote(writer))

        async def handle_client(reader: asyncio.StreamReader, writer: asyncio.StreamWriter) -> None:
            logger = get_logger(0)
            remote = rfb_format_remote(writer)
            logger.info("%s [entry]: Connected client", remote)
            try:
                sock = writer.get_extra_info("socket")
                if no_delay:
                    sock.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
                if keepalive_enabled:
                    # https://www.tldp.org/HOWTO/html_single/TCP-Keepalive-HOWTO/#setsockopt
                    # https://blog.cloudflare.com/when-tcp-sockets-refuse-to-die
                    sock.setsockopt(socket.SOL_SOCKET, socket.SO_KEEPALIVE, 1)
                    sock.setsockopt(socket.IPPROTO_TCP, socket.TCP_KEEPIDLE, keepalive_idle)
                    sock.setsockopt(socket.IPPROTO_TCP, socket.TCP_KEEPINTVL, keepalive_interval)
                    sock.setsockopt(socket.IPPROTO_TCP, socket.TCP_KEEPCNT, keepalive_count)
                    timeout = (keepalive_idle + keepalive_interval * keepalive_count) * 1000  # Milliseconds
                    sock.setsockopt(socket.IPPROTO_TCP, socket.TCP_USER_TIMEOUT, timeout)  # type: ignore

                try:
                    async with kvmd.make_session("", "") as kvmd_session:
                        none_auth_only = await kvmd_session.auth.check()
                except (aiohttp.ClientError, asyncio.TimeoutError) as err:
                    logger.error("%s [entry]: Can't check KVMD auth mode: %s", remote, tools.efmt(err))
                    return

                await _Client(
                    reader=reader,
                    writer=writer,
                    tls_ciphers=tls_ciphers,
                    tls_timeout=tls_timeout,
                    x509_cert_path=x509_cert_path,
                    x509_key_path=x509_key_path,
                    desired_fps=desired_fps,
                    mouse_output=mouse_output,
                    keymap_name=keymap_name,
                    symmap=symmap,
                    kvmd=kvmd,
                    streamers=streamers,
                    vnc_credentials=(await self.__vnc_auth_manager.read_credentials())[0],
                    none_auth_only=none_auth_only,
                    vencrypt=vencrypt_enabled,
                    shared_params=shared_params,
                ).run()
            except Exception:
                logger.exception("%s [entry]: Unhandled exception in client task", remote)
            finally:
                await aiotools.shield_fg(cleanup_client(writer))

        self.__handle_client = handle_client

    async def __inner_run(self) -> None:
        if not (await self.__vnc_auth_manager.read_credentials())[1]:
            raise SystemExit(1)

        get_logger(0).info("Listening VNC on TCP [%s]:%d ...", self.__host, self.__port)
        (family, _, _, _, addr) = socket.getaddrinfo(self.__host, self.__port, type=socket.SOCK_STREAM)[0]
        with contextlib.closing(socket.socket(family, socket.SOCK_STREAM)) as sock:
            if family == socket.AF_INET6:
                sock.setsockopt(socket.IPPROTO_IPV6, socket.IPV6_V6ONLY, 0)
            sock.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
            sock.bind(addr)

            server = await asyncio.start_server(
                client_connected_cb=self.__handle_client,
                sock=sock,
                backlog=self.__max_clients,
            )
            async with server:
                await server.serve_forever()

    def run(self) -> None:
        aiotools.run(self.__inner_run())
        get_logger().info("Bye-bye")
