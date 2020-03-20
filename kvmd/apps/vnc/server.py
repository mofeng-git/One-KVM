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
import asyncio.queues
import socket
import dataclasses
import contextlib
import json

from typing import Dict
from typing import Optional

import aiohttp

from ...logging import get_logger

from ... import aiotools

from .rfb import RfbError
from .rfb import RfbClient

from .kvmd import KvmdClient

from .streamer import StreamerError
from .streamer import StreamerClient

from .render import make_text_jpeg


# =====
@dataclasses.dataclass()
class _SharedParams:
    width: int = dataclasses.field(default=800)
    height: int = dataclasses.field(default=600)
    name: str = dataclasses.field(default="Pi-KVM")


class _Client(RfbClient):  # pylint: disable=too-many-instance-attributes
    def __init__(
        self,
        reader: asyncio.StreamReader,
        writer: asyncio.StreamWriter,

        kvmd: KvmdClient,
        streamer: StreamerClient,

        desired_fps: int,
        symmap: Dict[int, str],

        shared_params: _SharedParams,
    ) -> None:

        super().__init__(reader, writer, **dataclasses.asdict(shared_params))

        self.__kvmd = kvmd
        self.__streamer = streamer
        self.__desired_fps = desired_fps
        self.__symmap = symmap
        self.__shared_params = shared_params

        self.__authorized = asyncio.Future()  # type: ignore
        self.__ws_connected = asyncio.Future()  # type: ignore
        self.__ws_writer_queue: asyncio.queues.Queue = asyncio.Queue()

        self.__fb_requested = False
        self.__fb_stub_text = ""
        self.__fb_stub_quality = 0

        # Эти состояния шарить не обязательно - бекенд исключает дублирующиеся события.
        # Все это нужно только чтобы не посылать лишние жсоны в сокет KVMD
        self.__mouse_buttons: Dict[str, Optional[bool]] = {"left": None, "right": None, "middle": None}
        self.__mouse_move = {"x": -1, "y": -1}

    # =====

    async def run(self) -> None:
        await self._run(
            kvmd=self.__kvmd_task_loop(),
            streamer=self.__streamer_task_loop(),
        )

    # =====

    async def __kvmd_task_loop(self) -> None:
        logger = get_logger(0)

        await self.__authorized
        (user, passwd) = self.__authorized.result()

        async with self.__kvmd.ws(user, passwd) as ws:
            logger.info("[kvmd] Client %s: Connected to KVMD websocket", self._remote)
            self.__ws_connected.set_result(None)

            receive_task: Optional[asyncio.Task] = None
            writer_task: Optional[asyncio.Task] = None
            try:
                while True:
                    if receive_task is None:
                        receive_task = asyncio.create_task(ws.receive())
                    if writer_task is None:
                        writer_task = asyncio.create_task(self.__ws_writer_queue.get())

                    done = (await aiotools.wait_first(receive_task, writer_task))[0]

                    if receive_task in done:
                        msg = receive_task.result()
                        if msg.type == aiohttp.WSMsgType.TEXT:
                            await self.__process_ws_event(json.loads(msg.data))
                        elif msg.type == aiohttp.WSMsgType.CLOSE:
                            raise RfbError("KVMD closed the wesocket (it may have been stopped)")
                        else:
                            raise RuntimeError(f"Unhandled WS message type: {msg!r}")
                        receive_task = None

                    if writer_task in done:
                        await ws.send_str(json.dumps(writer_task.result()))
                        writer_task = None
            finally:
                if receive_task:
                    receive_task.cancel()
                if writer_task:
                    writer_task.cancel()

    async def __process_ws_event(self, event: Dict) -> None:
        if event["event_type"] == "info_state":
            host = event["event"]["meta"].get("server", {}).get("host")
            if isinstance(host, str):
                name = f"Pi-KVM: {host}"
                async with self._lock:
                    if self._encodings.has_rename:
                        await self._send_rename(name)
                self.__shared_params.name = name

        elif event["event_type"] == "hid_state":
            async with self._lock:
                if self._encodings.has_leds_state:
                    await self._send_leds_state(**event["event"]["keyboard"]["leds"])

    # =====

    async def __streamer_task_loop(self) -> None:
        logger = get_logger(0)
        await self.__ws_connected
        while True:
            try:
                streaming = False
                async for (online, width, height, jpeg) in self.__streamer.read():
                    if not streaming:
                        logger.info("[streamer] Client %s: Streaming ...", self._remote)
                        streaming = True
                    if online:
                        await self.__send_fb_real(width, height, jpeg)
                    else:
                        await self.__send_fb_stub("No signal")
            except StreamerError as err:
                logger.info("[streamer] Client %s: Waiting for stream: %s", self._remote, str(err))
                await self.__send_fb_stub("Waiting for stream ...")
                await asyncio.sleep(1)

    async def __send_fb_real(self, width: int, height: int, jpeg: bytes) -> None:
        async with self._lock:
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
            await self._lock.acquire()
        try:
            if self.__fb_requested and (self.__fb_stub_text != text or self.__fb_stub_quality != self._encodings.tight_jpeg_quality):
                await self._send_fb(await make_text_jpeg(self._width, self._height, self._encodings.tight_jpeg_quality, text))
                self.__fb_stub_text = text
                self.__fb_stub_quality = self._encodings.tight_jpeg_quality
                self.__fb_requested = False
        finally:
            if not no_lock:
                self._lock.release()

    # =====

    async def _authorize(self, user: str, passwd: str) -> bool:
        if (await self.__kvmd.authorize(user, passwd)):
            self.__authorized.set_result((user, passwd))
            return True
        return False

    async def _on_key_event(self, code: int, state: bool) -> None:
        if (web_name := self.__symmap.get(code)) is not None:  # noqa: E203,E231
            await self.__ws_writer_queue.put({
                "event_type": "key",
                "event": {"key": web_name, "state": state},
            })

    async def _on_pointer_event(self, buttons: Dict[str, bool], wheel: Dict[str, int], move: Dict[str, int]) -> None:
        for (button, state) in buttons.items():
            if self.__mouse_buttons[button] != state:
                await self.__ws_writer_queue.put({
                    "event_type": "mouse_button",
                    "event": {"button": button, "state": state},
                })
                self.__mouse_buttons[button] = state

        if wheel["x"] or wheel["y"]:
            await self.__ws_writer_queue.put({
                "event_type": "mouse_wheel",
                "event": {"delta": wheel},
            })

        if self.__mouse_move != move:
            await self.__ws_writer_queue.put({
                "event_type": "mouse_move",
                "event": {"to": move},
            })
            self.__mouse_move = move

    async def _on_cut_event(self, text: str) -> None:
        pass  # print("CutEvent", text)  # TODO

    async def _on_set_encodings(self) -> None:
        assert self.__authorized.done()
        (user, passwd) = self.__authorized.result()
        get_logger(0).info("[main] Client %s: Applying streamer params: quality=%d%%; desired_fps=%d ...",
                           self._remote, self._encodings.tight_jpeg_quality, self.__desired_fps)
        await self.__kvmd.set_streamer_params(user, passwd, self._encodings.tight_jpeg_quality, self.__desired_fps)

    async def _on_fb_update_request(self) -> None:
        self.__fb_requested = True


# =====
class VncServer:
    def __init__(
        self,
        host: str,
        port: int,
        max_clients: int,

        kvmd: KvmdClient,
        streamer: StreamerClient,

        desired_fps: int,
        symmap: Dict[int, str],
    ) -> None:

        self.__host = host
        self.__port = port
        self.__max_clients = max_clients

        self.__client_kwargs = {
            "kvmd": kvmd,
            "streamer": streamer,
            "desired_fps": desired_fps,
            "symmap": symmap,
            "shared_params": _SharedParams(),
        }

    def run(self) -> None:
        logger = get_logger(0)
        logger.info("Listening VNC on TCP [%s]:%d ...", self.__host, self.__port)

        with contextlib.closing(socket.socket(socket.AF_INET6, socket.SOCK_STREAM)) as sock:
            sock.setsockopt(socket.IPPROTO_IPV6, socket.IPV6_V6ONLY, False)
            sock.bind((self.__host, self.__port))

            loop = asyncio.get_event_loop()
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
                tasks = asyncio.Task.all_tasks()
                for task in tasks:
                    task.cancel()
                loop.run_until_complete(asyncio.gather(*tasks, return_exceptions=True))
                loop.close()
                logger.info("Bye-bye")

    async def __handle_client(self, reader: asyncio.StreamReader, writer: asyncio.StreamWriter) -> None:
        await _Client(reader, writer, **self.__client_kwargs).run()  # type: ignore
