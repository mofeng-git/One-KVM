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


import os
import asyncio
import socket
import dataclasses
import contextlib

from typing import Dict
from typing import Optional

import aiohttp

from ...logging import get_logger

from ...keyboard.keysym import switch_symmap_modifiers
from ...keyboard.keysym import build_symmap

from ...clients.kvmd import KvmdClientWs
from ...clients.kvmd import KvmdClientSession
from ...clients.kvmd import KvmdClient

from ...clients.streamer import StreamerError
from ...clients.streamer import StreamerClient

from .rfb import RfbClient
from .rfb.stream import rfb_format_remote
from .rfb.stream import rfb_close_writer
from .rfb.errors import RfbError

from .vncauth import VncAuthKvmdCredentials
from .vncauth import VncAuthManager

from .render import make_text_jpeg


# =====
@dataclasses.dataclass()
class _SharedParams:
    width: int = dataclasses.field(default=800)
    height: int = dataclasses.field(default=600)
    name: str = dataclasses.field(default="Pi-KVM")


class _Client(RfbClient):  # pylint: disable=too-many-instance-attributes
    def __init__(  # pylint: disable=too-many-arguments
        self,
        reader: asyncio.StreamReader,
        writer: asyncio.StreamWriter,
        tls_ciphers: str,
        tls_timeout: float,

        desired_fps: int,
        keymap_name: str,
        symmap: Dict[int, Dict[int, str]],

        kvmd: KvmdClient,
        streamer: StreamerClient,

        vnc_credentials: Dict[str, VncAuthKvmdCredentials],
        none_auth_only: bool,
        shared_params: _SharedParams,
    ) -> None:

        self.__vnc_credentials = vnc_credentials

        super().__init__(
            reader=reader,
            writer=writer,
            tls_ciphers=tls_ciphers,
            tls_timeout=tls_timeout,
            vnc_passwds=list(vnc_credentials),
            none_auth_only=none_auth_only,
            **dataclasses.asdict(shared_params),
        )

        self.__desired_fps = desired_fps
        self.__keymap_name = keymap_name
        self.__symmap = symmap

        self.__kvmd = kvmd
        self.__streamer = streamer

        self.__shared_params = shared_params

        self.__authorized = asyncio.Future()  # type: ignore
        self.__ws_connected = asyncio.Future()  # type: ignore
        self.__kvmd_session: Optional[KvmdClientSession] = None
        self.__kvmd_ws: Optional[KvmdClientWs] = None

        self.__fb_requested = False
        self.__fb_stub_text = ""
        self.__fb_stub_quality = 0

        # Эти состояния шарить не обязательно - бекенд исключает дублирующиеся события.
        # Все это нужно только чтобы не посылать лишние жсоны в сокет KVMD
        self.__mouse_buttons: Dict[str, Optional[bool]] = dict.fromkeys(["left", "right", "middle"], None)
        self.__mouse_move = {"x": -1, "y": -1}

        self.__lock = asyncio.Lock()

        self.__modifiers = 0

    # =====

    async def run(self) -> None:
        try:
            await self._run(
                kvmd=self.__kvmd_task_loop(),
                streamer=self.__streamer_task_loop(),
            )
        finally:
            if self.__kvmd_session:
                await self.__kvmd_session.close()
                self.__kvmd_session = None

    # =====

    async def __kvmd_task_loop(self) -> None:
        logger = get_logger(0)
        await self.__authorized
        assert self.__kvmd_session
        try:
            async with self.__kvmd_session.ws() as self.__kvmd_ws:
                logger.info("[kvmd] %s: Connected to KVMD websocket", self._remote)
                self.__ws_connected.set_result(None)
                async for event in self.__kvmd_ws.communicate():
                    await self.__process_ws_event(event)
                raise RfbError("KVMD closes the websocket (the server may have been stopped)")
        finally:
            self.__kvmd_ws = None

    async def __process_ws_event(self, event: Dict) -> None:
        if event["event_type"] == "info_meta_state":
            try:
                host = event["event"]["server"]["host"]
            except Exception:
                host = None
            else:
                if isinstance(host, str):
                    name = f"Pi-KVM: {host}"
                    async with self.__lock:
                        if self._encodings.has_rename:
                            await self._send_rename(name)
                    self.__shared_params.name = name

        elif event["event_type"] == "hid_state":
            async with self.__lock:
                if self._encodings.has_leds_state:
                    await self._send_leds_state(**event["event"]["keyboard"]["leds"])

    # =====

    async def __streamer_task_loop(self) -> None:
        logger = get_logger(0)
        await self.__ws_connected
        while True:
            try:
                streaming = False
                async for (online, width, height, jpeg) in self.__streamer.read_stream():
                    if not streaming:
                        logger.info("[streamer] %s: Streaming ...", self._remote)
                        streaming = True
                    if online:
                        await self.__send_fb_real(width, height, jpeg)
                    else:
                        await self.__send_fb_stub("No signal")
            except StreamerError as err:
                logger.info("[streamer] %s: Waiting for stream: %s", self._remote, err)
                await self.__send_fb_stub("Waiting for stream ...")
                await asyncio.sleep(1)

    async def __send_fb_real(self, width: int, height: int, jpeg: bytes) -> None:
        async with self.__lock:
            if self.__fb_requested:
                if (self._width, self._height) != (width, height):
                    self.__shared_params.width = width
                    self.__shared_params.height = height
                    if not self._encodings.has_resize:
                        msg = f"Resoultion changed: {self._width}x{self._height} -> {width}x{height}\nPlease reconnect"
                        await self.__send_fb_stub(msg, no_lock=True)
                        return
                    await self._send_resize(width, height)
                await self._send_fb(jpeg)
                self.__fb_stub_text = ""
                self.__fb_stub_quality = 0
                self.__fb_requested = False

    async def __send_fb_stub(self, text: str, no_lock: bool=False) -> None:
        if not no_lock:
            await self.__lock.acquire()
        try:
            if self.__fb_requested and (self.__fb_stub_text != text or self.__fb_stub_quality != self._encodings.tight_jpeg_quality):
                await self._send_fb(await make_text_jpeg(self._width, self._height, self._encodings.tight_jpeg_quality, text))
                self.__fb_stub_text = text
                self.__fb_stub_quality = self._encodings.tight_jpeg_quality
                self.__fb_requested = False
        finally:
            if not no_lock:
                self.__lock.release()

    # =====

    async def _authorize_userpass(self, user: str, passwd: str) -> bool:
        self.__kvmd_session = self.__kvmd.make_session(user, passwd)
        if (await self.__kvmd_session.auth.check()):
            self.__authorized.set_result(None)
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
        (is_modifier, self.__modifiers) = switch_symmap_modifiers(self.__modifiers, code, state)
        if self.__kvmd_ws:
            web_keys = self.__symmap.get(code)
            if web_keys:
                if is_modifier:
                    web_key = web_keys.get(0)
                else:
                    web_key = web_keys.get(self.__modifiers)
                    if web_key is None:
                        web_key = web_keys.get(0)
                if web_key is not None:
                    await self.__kvmd_ws.send_key_event(web_key, state)

    async def _on_pointer_event(self, buttons: Dict[str, bool], wheel: Dict[str, int], move: Dict[str, int]) -> None:
        if self.__kvmd_ws:
            for (button, state) in buttons.items():
                if self.__mouse_buttons[button] != state:
                    await self.__kvmd_ws.send_mouse_button_event(button, state)
                    self.__mouse_buttons[button] = state

            if wheel["x"] or wheel["y"]:
                await self.__kvmd_ws.send_mouse_wheel_event(wheel["x"], wheel["y"])

            if self.__mouse_move != move:
                await self.__kvmd_ws.send_mouse_move_event(move["x"], move["y"])
                self.__mouse_move = move

    async def _on_cut_event(self, text: str) -> None:
        assert self.__authorized.done()
        assert self.__kvmd_session
        logger = get_logger(0)
        logger.info("[main] %s: Printing %d characters ...", self._remote, len(text))
        try:
            (default, available) = await self.__kvmd_session.hid.get_keymaps()
            await self.__kvmd_session.hid.print(
                text=text,
                limit=0,
                keymap_name=(self.__keymap_name if self.__keymap_name in available else default),
            )
        except Exception:
            logger.exception("[main] %s: Can't print characters", self._remote)

    async def _on_set_encodings(self) -> None:
        assert self.__authorized.done()
        assert self.__kvmd_session
        has_quality = (await self.__kvmd_session.streamer.get_state())["features"]["quality"]
        quality = (self._encodings.tight_jpeg_quality if has_quality else None)
        get_logger(0).info("[main] %s: Applying streamer params: quality=%s; desired_fps=%d ...",
                           self._remote, quality, self.__desired_fps)
        await self.__kvmd_session.streamer.set_params(quality, self.__desired_fps)

    async def _on_fb_update_request(self) -> None:
        async with self.__lock:
            self.__fb_requested = True


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

        desired_fps: int,
        keymap_path: str,

        kvmd: KvmdClient,
        streamer: StreamerClient,
        vnc_auth_manager: VncAuthManager,
    ) -> None:

        self.__host = host
        self.__port = port
        self.__max_clients = max_clients

        keymap_name = os.path.basename(keymap_path)
        symmap = build_symmap(keymap_path)

        self.__vnc_auth_manager = vnc_auth_manager

        shared_params = _SharedParams()

        async def handle_client(reader: asyncio.StreamReader, writer: asyncio.StreamWriter) -> None:
            logger = get_logger(0)
            remote = rfb_format_remote(writer)
            logger.info("[entry] %s: Connected client", remote)
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
                    sock.setsockopt(socket.IPPROTO_TCP, socket.TCP_USER_TIMEOUT, timeout)

                try:
                    async with kvmd.make_session("", "") as kvmd_session:
                        none_auth_only = await kvmd_session.auth.check()
                except (aiohttp.ClientError, asyncio.TimeoutError) as err:
                    logger.error("[entry] %s: Can't check KVMD auth mode: %s: %s", remote, type(err).__name__, err)
                    return

                await _Client(
                    reader=reader,
                    writer=writer,
                    tls_ciphers=tls_ciphers,
                    tls_timeout=tls_timeout,
                    desired_fps=desired_fps,
                    keymap_name=keymap_name,
                    symmap=symmap,
                    kvmd=kvmd,
                    streamer=streamer,
                    vnc_credentials=(await self.__vnc_auth_manager.read_credentials())[0],
                    none_auth_only=none_auth_only,
                    shared_params=shared_params,
                ).run()
            except Exception:
                logger.exception("[entry] %s: Unhandled exception in client task", remote)
            finally:
                if (await rfb_close_writer(writer)):
                    logger.info("[entry] %s: Connection is closed in an emergency", remote)

        self.__handle_client = handle_client

    def run(self) -> None:
        logger = get_logger(0)
        loop = asyncio.get_event_loop()
        try:
            if not loop.run_until_complete(self.__vnc_auth_manager.read_credentials())[1]:
                raise SystemExit(1)

            logger.info("Listening VNC on TCP [%s]:%d ...", self.__host, self.__port)

            with contextlib.closing(socket.socket(socket.AF_INET6, socket.SOCK_STREAM)) as sock:
                sock.setsockopt(socket.IPPROTO_IPV6, socket.IPV6_V6ONLY, 0)
                sock.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
                sock.bind((self.__host, self.__port))

                server = loop.run_until_complete(asyncio.start_server(
                    client_connected_cb=self.__handle_client,
                    sock=sock,
                    backlog=self.__max_clients,
                    loop=loop,
                ))

                try:
                    loop.run_forever()
                except (SystemExit, KeyboardInterrupt):
                    pass
                finally:
                    server.close()
                    loop.run_until_complete(server.wait_closed())
        finally:
            tasks = asyncio.Task.all_tasks()
            for task in tasks:
                task.cancel()
            loop.run_until_complete(asyncio.gather(*tasks, return_exceptions=True))
            loop.close()
            logger.info("Bye-bye")
