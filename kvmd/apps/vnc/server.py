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
import async_lru

from ...logging import get_logger

from ...keyboard.keysym import SymmapModifiers
from ...keyboard.keysym import build_symmap
from ...keyboard.mappings import EvdevModifiers
from ...keyboard.mappings import X11Modifiers
from ...keyboard.mappings import AT1_TO_EVDEV
from ...keyboard.magic import BaseMagicHandler

from ...mouse import MOUSE_TO_EVDEV

from ...clients.kvmd import KvmdClientWs
from ...clients.kvmd import KvmdClientSession
from ...clients.kvmd import KvmdClient

from ...clients.streamer import StreamerError
from ...clients.streamer import StreamerPermError
from ...clients.streamer import StreamerFormats
from ...clients.streamer import BaseStreamerClient

from ... import tools
from ... import aiotools
from ... import network

from .rfb import RfbClient
from .rfb.stream import rfb_format_remote
from .rfb.errors import RfbError

from .render import make_text_jpeg


# =====
@dataclasses.dataclass()
class _SharedParams:
    width: int = dataclasses.field(default=800)
    height: int = dataclasses.field(default=600)
    name: str = dataclasses.field(default="PiKVM")


class _Client(RfbClient, BaseMagicHandler):  # pylint: disable=too-many-instance-attributes
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
        symmap: dict[int, dict[int, int]],
        scroll_rate: int,

        kvmd: KvmdClient,
        streamers: list[BaseStreamerClient],

        vncpasses: set[str],
        vencrypt: bool,
        none_auth_only: bool,

        shared_params: _SharedParams,
    ) -> None:

        RfbClient.__init__(
            self,
            reader=reader,
            writer=writer,
            tls_ciphers=tls_ciphers,
            tls_timeout=tls_timeout,
            x509_cert_path=x509_cert_path,
            x509_key_path=x509_key_path,
            scroll_rate=scroll_rate,
            vncpasses=vncpasses,
            vencrypt=vencrypt,
            none_auth_only=none_auth_only,
            **dataclasses.asdict(shared_params),
        )
        BaseMagicHandler.__init__(self)

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
        self.__mouse_buttons: dict[str, (bool | None)] = dict.fromkeys(MOUSE_TO_EVDEV, None)
        self.__mouse_move = (-1, -1)  # (X, Y)
        self.__modifiers = 0

        self.__clipboard = ""

        self.__info_host = ""
        self.__info_switch_units = 0
        self.__info_switch_active = ""

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
        if event_type == "info":
            if "meta" in event:
                host = ""
                try:
                    if isinstance(event["meta"]["server"]["host"], str):
                        host = event["meta"]["server"]["host"].strip()
                except Exception:
                    pass
                self.__info_host = host
                await self.__update_info()

        elif event_type == "switch":
            if "model" in event:
                self.__info_switch_units = len(event["model"]["units"])
            if "summary" in event:
                self.__info_switch_active = event["summary"]["active_id"]
            if "model" in event or "summary" in event:
                await self.__update_info()

        elif event_type == "hid":
            if (
                self._encodings.has_leds_state
                and ("keyboard" in event)
                and ("leds" in event["keyboard"])
            ):
                await self._send_leds_state(**event["keyboard"]["leds"])

    async def __update_info(self) -> None:
        info: list[str] = []
        if self.__info_switch_units > 0:
            info.append("Port " + (self.__info_switch_active or "not selected"))
        if self.__info_host:
            info.append(self.__info_host)
        info.append("PiKVM")
        self.__shared_params.name = " | ".join(info)
        if self._encodings.has_rename:
            await self._send_rename(self.__shared_params.name)

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
                        await self.__queue_frame(frame)
            except StreamerError as ex:
                if isinstance(ex, StreamerPermError):
                    streamer = self.__get_default_streamer()
                    logger.info("%s [streamer]: Permanent error: %s; switching to %s ...", self._remote, ex, streamer)
                else:
                    logger.info("%s [streamer]: Waiting for stream: %s", self._remote, ex)
                await self.__queue_frame("Waiting for stream ...")
                await asyncio.sleep(1)

    def __get_preferred_streamer(self) -> BaseStreamerClient:
        formats = {
            StreamerFormats.JPEG: "has_tight",
            StreamerFormats.H264: "has_h264",
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
            "format": StreamerFormats.JPEG,
        }

    async def __fb_sender_task_loop(self) -> None:  # pylint: disable=too-many-branches
        last: (dict | None) = None
        async for _ in self._send_fb_allowed():
            while True:
                frame = await self.__fb_queue.get()
                if (
                    last is None  # pylint: disable=too-many-boolean-expressions
                    or frame["format"] == StreamerFormats.JPEG
                    or last["format"] != frame["format"]
                    or (frame["format"] == StreamerFormats.H264 and (
                        frame["key"]
                        or last["width"] != frame["width"]
                        or last["height"] != frame["height"]
                        or len(last["data"]) + len(frame["data"]) > 4194304
                    ))
                ):
                    self.__fb_has_key = (frame["format"] == StreamerFormats.H264 and frame["key"])
                    last = frame
                    if self.__fb_queue.qsize() == 0:
                        break
                    continue
                assert frame["format"] == StreamerFormats.H264
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

            if last["format"] == StreamerFormats.JPEG:
                await self._send_fb_jpeg(last["data"])
            elif last["format"] == StreamerFormats.H264:
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
        self.__kvmd_session = self.__kvmd.make_session()
        if (await self.__kvmd_session.auth.check(user, passwd)):
            self.__stage1_authorized.set_passed()
            return True
        return False

    async def _on_authorized_vncpass(self) -> None:
        self.__kvmd_session = self.__kvmd.make_session()
        self.__stage1_authorized.set_passed()

    async def _authorize_none(self) -> bool:
        return (await self._authorize_userpass("", ""))

    # =====

    async def _on_key_event(self, code: int, state: bool) -> None:
        assert self.__stage1_authorized.is_passed()

        is_modifier = self.__switch_modifiers_x11(code, state)
        variants = self.__symmap.get(code)
        fake_shift = False

        if variants:
            if is_modifier:
                key = variants.get(0)
            else:
                key = variants.get(self.__modifiers)
                if key is None:
                    key = variants.get(0)

                if key is None and self.__modifiers == 0 and SymmapModifiers.SHIFT in variants:
                    # JUMP doesn't send shift events:
                    #   - https://github.com/pikvm/pikvm/issues/820
                    key = variants[SymmapModifiers.SHIFT]
                    fake_shift = True

            if key:
                if fake_shift:
                    await self._magic_handle_key(EvdevModifiers.SHIFT_LEFT, True)
                await self._magic_handle_key(key, state)
                if fake_shift:
                    await self._magic_handle_key(EvdevModifiers.SHIFT_LEFT, False)

    async def _on_ext_key_event(self, code: int, state: bool) -> None:
        assert self.__stage1_authorized.is_passed()
        key = AT1_TO_EVDEV.get(code, 0)
        if key:
            self.__switch_modifiers_evdev(key, state)  # Предполагаем, что модификаторы всегда известны
            await self._magic_handle_key(key, state)

    def __switch_modifiers_x11(self, key: int, state: bool) -> bool:
        mod = 0
        if key in X11Modifiers.SHIFTS:
            mod = SymmapModifiers.SHIFT
        elif key == X11Modifiers.ALTGR:
            mod = SymmapModifiers.ALTGR
        elif key in X11Modifiers.CTRLS:
            mod = SymmapModifiers.CTRL
        if mod == 0:
            return False
        if state:
            self.__modifiers |= mod
        else:
            self.__modifiers &= ~mod
        return True

    def __switch_modifiers_evdev(self, key: int, state: bool) -> bool:
        mod = 0
        if key in EvdevModifiers.SHIFTS:
            mod = SymmapModifiers.SHIFT
        elif key == EvdevModifiers.ALT_RIGHT:
            mod = SymmapModifiers.ALTGR
        elif key in EvdevModifiers.CTRLS:
            mod = SymmapModifiers.CTRL
        if mod == 0:
            return False
        if state:
            self.__modifiers |= mod
        else:
            self.__modifiers &= ~mod
        return True

    async def _on_magic_switch_prev(self) -> None:
        assert self.__kvmd_session
        if self.__info_switch_units > 0:
            get_logger(0).info("%s [main]: Switching port to the previous one ...", self._remote)
            await self.__kvmd_session.switch.set_active_prev()

    async def _on_magic_switch_next(self) -> None:
        assert self.__kvmd_session
        if self.__info_switch_units > 0:
            get_logger(0).info("%s [main]: Switching port to the next one ...", self._remote)
            await self.__kvmd_session.switch.set_active_next()

    async def _on_magic_switch_port(self, first: int, second: int) -> bool:
        assert self.__kvmd_session
        if self.__info_switch_units <= 0:
            return True
        elif 1 <= self.__info_switch_units <= 2:
            port = float(first)
        else:  # self.__info_switch_units > 2:
            if second < 0:
                return False  # Wait for the second key
            port = (first + 1) + (second + 1) / 10
        get_logger(0).info("%s [main]: Switching port to %s ...", self._remote, port)
        await self.__kvmd_session.switch.set_active(port)
        return True

    async def _on_magic_clipboard_print(self) -> None:
        assert self.__kvmd_session
        if self.__clipboard:
            logger = get_logger(0)
            logger.info("%s [main]: Printing %d characters ...", self._remote, len(self.__clipboard))
            try:
                (keymap_name, available) = await self.__kvmd_session.hid.get_keymaps()
                if self.__keymap_name in available:
                    keymap_name = self.__keymap_name
                await self.__kvmd_session.hid.print(self.__clipboard, 0, keymap_name)
            except Exception:
                logger.exception("%s [main]: Can't print characters", self._remote)

    async def _on_magic_key_proxy(self, key: int, state: bool) -> None:
        if self.__kvmd_ws:
            await self.__kvmd_ws.send_key_event(key, state)

    # =====

    async def _on_pointer_event(self, buttons: dict[str, bool], wheel: tuple[int, int], move: tuple[int, int]) -> None:
        assert self.__stage1_authorized.is_passed()
        if self.__kvmd_ws:
            if wheel[0] or wheel[1]:
                await self.__kvmd_ws.send_mouse_wheel_event(*wheel)

            if self.__mouse_move != move:
                await self.__kvmd_ws.send_mouse_move_event(*move)
                self.__mouse_move = move

            for (button, state) in buttons.items():
                if self.__mouse_buttons[button] != state:
                    await self.__kvmd_ws.send_mouse_button_event(MOUSE_TO_EVDEV[button], state)
                    self.__mouse_buttons[button] = state

    async def _on_cut_event(self, text: str) -> None:
        assert self.__stage1_authorized.is_passed()
        self.__clipboard = text

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

        vncpass_enabled: bool,
        vncpass_path: str,
        vencrypt_enabled: bool,

        desired_fps: int,
        mouse_output: str,
        keymap_path: str,
        scroll_rate: int,

        kvmd: KvmdClient,
        streamers: list[BaseStreamerClient],
    ) -> None:

        self.__host = network.get_listen_host(host)
        self.__port = port
        self.__max_clients = max_clients

        keymap_name = os.path.basename(keymap_path)
        symmap = build_symmap(keymap_path)

        self.__vncpass_enabled = vncpass_enabled
        self.__vncpass_path = vncpass_path

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
                    async with kvmd.make_session() as kvmd_session:
                        none_auth_only = await kvmd_session.auth.check("", "")
                except (aiohttp.ClientError, asyncio.TimeoutError) as ex:
                    logger.error("%s [entry]: Can't check KVMD auth mode: %s", remote, tools.efmt(ex))
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
                    scroll_rate=scroll_rate,
                    kvmd=kvmd,
                    streamers=streamers,
                    vncpasses=(await self.__read_vncpasses()),
                    vencrypt=vencrypt_enabled,
                    none_auth_only=none_auth_only,
                    shared_params=shared_params,
                ).run()
            except Exception:
                logger.exception("%s [entry]: Unhandled exception in client task", remote)
            finally:
                await aiotools.shield_fg(cleanup_client(writer))

        self.__handle_client = handle_client

    async def __inner_run(self) -> None:
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

    @async_lru.alru_cache(maxsize=1, ttl=1)
    async def __read_vncpasses(self) -> set[str]:
        if self.__vncpass_enabled:
            try:
                vncpasses: set[str] = set()
                for (_, line) in tools.passwds_splitted(await aiotools.read_file(self.__vncpass_path)):
                    if " -> " in line:  # Compatibility with old ipmipasswd file format
                        line = line.split(" -> ", 1)[0]
                    if len(line.strip()) > 0:
                        vncpasses.add(line)
                return vncpasses
            except Exception:
                get_logger(0).exception("Unhandled exception while reading VNCAuth passwd file")
        return set()

    def run(self) -> None:
        aiotools.run(self.__inner_run())
        get_logger().info("Bye-bye")
