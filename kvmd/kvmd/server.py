import os
import signal
import asyncio
import time

from typing import List
from typing import Set
from typing import Callable
from typing import Optional

import aiohttp.web

from .atx import Atx
from .streamer import Streamer
from .ps2 import Ps2Keyboard

from .logging import get_logger


# =====
def _system_task(method: Callable) -> Callable:
    async def wrap(self: "Server") -> None:
        try:
            await method(self)
        except asyncio.CancelledError:
            pass
        except Exception:
            get_logger().exception("Unhandled exception, killing myself ...")
            os.kill(os.getpid(), signal.SIGTERM)
    return wrap


class Server:  # pylint: disable=too-many-instance-attributes
    def __init__(
        self,
        atx: Atx,
        streamer: Streamer,
        keyboard: Ps2Keyboard,
        heartbeat: float,
        atx_leds_poll: float,
        video_shutdown_delay: float,
        loop: asyncio.AbstractEventLoop,
    ) -> None:

        self.__atx = atx
        self.__streamer = streamer
        self.__heartbeat = heartbeat
        self.__keyboard = keyboard
        self.__video_shutdown_delay = video_shutdown_delay
        self.__atx_leds_poll = atx_leds_poll
        self.__loop = loop

        self.__sockets: Set[aiohttp.web.WebSocketResponse] = set()
        self.__sockets_lock = asyncio.Lock()

        self.__system_tasks: List[asyncio.Task] = []

        self.__restart_video = False

    def run(self, host: str, port: int) -> None:
        self.__keyboard.start()

        app = aiohttp.web.Application(loop=self.__loop)
        app.router.add_get("/", self.__root_handler)
        app.router.add_get("/ws", self.__ws_handler)
        app.on_shutdown.append(self.__on_shutdown)
        app.on_cleanup.append(self.__on_cleanup)

        self.__system_tasks.extend([
            self.__loop.create_task(self.__keyboard_watchdog()),
            self.__loop.create_task(self.__stream_controller()),
            self.__loop.create_task(self.__poll_dead_sockets()),
            self.__loop.create_task(self.__poll_atx_leds()),
        ])

        aiohttp.web.run_app(app, host=host, port=port, print=self.__run_app_print)

    async def __root_handler(self, _: aiohttp.web.Request) -> aiohttp.web.Response:
        return aiohttp.web.Response(text="OK")

    async def __ws_handler(self, request: aiohttp.web.Request) -> aiohttp.web.WebSocketResponse:
        ws = aiohttp.web.WebSocketResponse(heartbeat=self.__heartbeat)
        await ws.prepare(request)
        await self.__register_socket(ws)
        async for msg in ws:
            if msg.type == aiohttp.web.WSMsgType.TEXT:
                retval = await self.__execute_command(msg.data)
                if retval:
                    await ws.send_str(retval)
            else:
                break
        return ws

    def __run_app_print(self, text: str) -> None:
        logger = get_logger()
        for line in text.strip().splitlines():
            logger.info(line.strip())

    async def __on_shutdown(self, _: aiohttp.web.Application) -> None:
        logger = get_logger(0)

        logger.info("Cancelling system tasks ...")
        for task in self.__system_tasks:
            task.cancel()
        await asyncio.gather(*self.__system_tasks)

        logger.info("Disconnecting clients ...")
        for ws in list(self.__sockets):
            await self.__remove_socket(ws)

    async def __on_cleanup(self, _: aiohttp.web.Application) -> None:
        if self.__keyboard.is_alive():
            self.__keyboard.stop()
        if self.__streamer.is_running():
            await self.__streamer.stop()

    @_system_task
    async def __keyboard_watchdog(self) -> None:
        while self.__keyboard.is_alive():
            await asyncio.sleep(0.1)
        raise RuntimeError("Keyboard dead")

    @_system_task
    async def __stream_controller(self) -> None:
        prev = 0
        shutdown_at = 0.0

        while True:
            cur = len(self.__sockets)
            if prev == 0 and cur > 0:
                if not self.__streamer.is_running():
                    await self.__streamer.start()
            elif prev > 0 and cur == 0:
                shutdown_at = time.time() + self.__video_shutdown_delay
            elif prev == 0 and cur == 0 and time.time() > shutdown_at:
                if self.__streamer.is_running():
                    await self.__streamer.stop()

            if self.__restart_video:
                if self.__streamer.is_running():
                    await self.__streamer.stop()
                    await self.__streamer.start()
                self.__restart_video = False

            prev = cur
            await asyncio.sleep(0.1)

    @_system_task
    async def __poll_dead_sockets(self) -> None:
        while True:
            for ws in list(self.__sockets):
                if ws.closed or not ws._req.transport:  # pylint: disable=protected-access
                    await self.__remove_socket(ws)
            await asyncio.sleep(0.1)

    @_system_task
    async def __poll_atx_leds(self) -> None:
        while True:
            if self.__sockets:
                await self.__broadcast("EVENT atx_leds %d %d" % (self.__atx.get_leds()))
            await asyncio.sleep(self.__atx_leds_poll)

    async def __broadcast(self, msg: str) -> None:
        await asyncio.gather(*[
            ws.send_str(msg)
            for ws in list(self.__sockets)
            if not ws.closed and ws._req.transport  # pylint: disable=protected-access
        ], return_exceptions=True)

    async def __execute_command(self, command: str) -> Optional[str]:
        (command, args) = (command.strip().split(" ", maxsplit=1) + [""])[:2]
        if command == "CLICK":
            method = {
                "power": self.__atx.click_power,
                "power_long": self.__atx.click_power_long,
                "reset": self.__atx.click_reset,
            }.get(args)
            if method:
                await method()
                return None
        elif command == "RESTART_VIDEO":
            self.__restart_video = True
            return None
        get_logger().warning("Received an incorrect command: %r", command)
        return "ERROR incorrect command"

    async def __register_socket(self, ws: aiohttp.web.WebSocketResponse) -> None:
        async with self.__sockets_lock:
            self.__sockets.add(ws)
            get_logger().info("Registered new client socket: remote=%s; id=%d; active=%d",
                              ws._req.remote, id(ws), len(self.__sockets))  # pylint: disable=protected-access

    async def __remove_socket(self, ws: aiohttp.web.WebSocketResponse) -> None:
        async with self.__sockets_lock:
            try:
                self.__sockets.remove(ws)
                get_logger().info("Removed client socket: remote=%s; id=%d; active=%d",
                                  ws._req.remote, id(ws), len(self.__sockets))  # pylint: disable=protected-access
                await ws.close()
            except Exception:
                pass
