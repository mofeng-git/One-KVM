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


import asyncio
import dataclasses

from aiohttp.web import Request
from aiohttp.web import WebSocketResponse

from ...logging import get_logger

from ... import tools
from ... import aiotools

from ...htserver import exposed_http
from ...htserver import exposed_ws
from ...htserver import WsSession
from ...htserver import HttpServer

from ...clients.streamer import StreamerError
from ...clients.streamer import StreamerPermError
from ...clients.streamer import StreamerFormats
from ...clients.streamer import BaseStreamerClient


# =====
@dataclasses.dataclass
class _Source:
    type:         str
    fmt:          str
    streamer:     BaseStreamerClient
    meta:         dict = dataclasses.field(default_factory=dict)
    clients:      dict[WsSession, "_Client"] = dataclasses.field(default_factory=dict)
    key_required: bool = dataclasses.field(default=False)

    def is_diff(self) -> bool:
        return StreamerFormats.is_diff(self.streamer.get_format())


@dataclasses.dataclass
class _Client:
    ws:     WsSession
    src:    _Source
    queue:  asyncio.Queue[dict]
    sender: (asyncio.Task | None) = dataclasses.field(default=None)


class MediaServer(HttpServer):
    __K_VIDEO = "video"

    __F_H264 = "h264"
    __F_JPEG = "jpeg"

    __Q_SIZE = 32

    def __init__(
        self,
        h264_streamer: (BaseStreamerClient | None),
        jpeg_streamer: (BaseStreamerClient | None),
    ) -> None:

        super().__init__()

        self.__srcs: list[_Source] = []
        if h264_streamer:
            self.__srcs.append(_Source(self.__K_VIDEO, self.__F_H264, h264_streamer, {"profile_level_id": "42E01F"}))
        if jpeg_streamer:
            self.__srcs.append(_Source(self.__K_VIDEO, self.__F_JPEG, jpeg_streamer))

    # =====

    @exposed_http("GET", "/ws")
    async def __ws_handler(self, req: Request) -> WebSocketResponse:
        async with self._ws_session(req) as ws:
            media: dict = {self.__K_VIDEO: {}}
            for src in self.__srcs:
                media[src.type][src.fmt] = src.meta
            await ws.send_event("media", media)
            return (await self._ws_loop(ws))

    @exposed_ws(0)
    async def __ws_bin_ping_handler(self, ws: WsSession, _: bytes) -> None:
        await ws.send_bin(255, b"")  # Ping-pong

    @exposed_ws(1)
    async def __ws_bin_key_handler(self, ws: WsSession, _: bytes) -> None:
        for src in self.__srcs:
            if ws in src.clients:
                if src.is_diff():
                    src.key_required = True
                break

    @exposed_ws("start")
    async def __ws_start_handler(self, ws: WsSession, event: dict) -> None:
        try:
            req_type = str(event.get("type"))
            req_fmt = str(event.get("format"))
        except Exception:
            return
        src: (_Source | None) = None
        for cand in self.__srcs:
            if ws in cand.clients:
                return  # Don't allow any double streaming
            if (cand.type, cand.fmt) == (req_type, req_fmt):
                src = cand
        if src:
            client = _Client(ws, src, asyncio.Queue(self.__Q_SIZE))
            client.sender = aiotools.create_deadly_task(str(ws), self.__sender(client))
            src.clients[ws] = client
            get_logger(0).info("Streaming %s to %s ...", src.streamer, ws)

    # =====

    async def _init_app(self) -> None:
        logger = get_logger(0)
        for src in self.__srcs:
            logger.info("Starting streamer %s ...", src.streamer)
            aiotools.create_deadly_task(str(src.streamer), self.__streamer(src))
        self._add_exposed(self)

    async def _on_shutdown(self) -> None:
        logger = get_logger(0)
        logger.info("Stopping system tasks ...")
        await aiotools.stop_all_deadly_tasks()
        logger.info("Disconnecting clients ...")
        await self._close_all_wss()
        logger.info("On-Shutdown complete")

    async def _on_ws_closed(self, ws: WsSession) -> None:
        for src in self.__srcs:
            client = src.clients.pop(ws, None)
            if client and client.sender:
                get_logger(0).info("Closed stream for %s", ws)
                client.sender.cancel()
                return

    # =====

    async def __sender(self, client: _Client) -> None:
        need_key = client.src.is_diff()
        if need_key:
            client.src.key_required = True
        has_key = False
        while True:
            frame = await client.queue.get()
            has_key = (not need_key or has_key or frame["key"])
            if has_key:
                try:
                    await client.ws.send_bin(1, frame["key"].to_bytes() + frame["data"])
                except Exception:
                    pass

    async def __streamer(self, src: _Source) -> None:
        logger = get_logger(0)
        while True:
            if len(src.clients) == 0:
                await asyncio.sleep(1)
                continue
            try:
                async with src.streamer.reading() as read_frame:
                    while len(src.clients) > 0:
                        frame = await read_frame(src.key_required)
                        if frame["key"]:
                            src.key_required = False
                        for client in src.clients.values():
                            try:
                                client.queue.put_nowait(frame)
                            except asyncio.QueueFull:
                                # Если какой-то из клиентов не справляется, очищаем ему очередь и запрашиваем кейфрейм.
                                # Я вижу у такой логики кучу минусов, хз как себя покажет, но лучше пока ничего не придумал.
                                tools.clear_queue(client.queue)
                                src.key_required = True
                            except Exception:
                                pass
            except StreamerError as ex:
                if isinstance(ex, StreamerPermError):
                    logger.exception("Streamer failed: %s", src.streamer)
                else:
                    logger.error("Streamer error: %s: %s", src.streamer, tools.efmt(ex))
            except Exception:
                get_logger(0).exception("Unexpected streamer error: %s", src.streamer)
            await asyncio.sleep(1)
