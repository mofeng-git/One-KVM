import asyncio
import argparse
import logging
import logging.config

from typing import List
from typing import Dict
from typing import Set
from typing import Callable
from typing import Optional

from contextlog import get_logger
from contextlog import patch_logging
from contextlog import patch_threading

from RPi import GPIO

import aiohttp

import yaml

from .atx import Atx
from .streamer import Streamer


# =====
def _system_task(method: Callable) -> Callable:
    async def wrap(self: "_Application") -> None:
        try:
            await method(self)
        except asyncio.CancelledError:
            pass
        except Exception:
            get_logger().exception("Unhandled exception")
            raise SystemExit(1)
    return wrap


class _Application:
    def __init__(self, config: Dict) -> None:
        self.__config = config

        self.__loop = asyncio.get_event_loop()
        self.__sockets: Set[aiohttp.web.WebSocketResponse] = set()
        self.__sockets_lock = asyncio.Lock()

        GPIO.setmode(GPIO.BCM)

        self.__atx = Atx(
            power_led=self.__config["atx"]["leds"]["pinout"]["power"],
            hdd_led=self.__config["atx"]["leds"]["pinout"]["hdd"],
            power_switch=self.__config["atx"]["switches"]["pinout"]["power"],
            reset_switch=self.__config["atx"]["switches"]["pinout"]["reset"],
            click_delay=self.__config["atx"]["switches"]["click_delay"],
            long_click_delay=self.__config["atx"]["switches"]["long_click_delay"],
        )

        self.__streamer = Streamer(
            cap_power=self.__config["video"]["pinout"]["cap"],
            vga_power=self.__config["video"]["pinout"]["vga"],
            sync_delay=self.__config["video"]["sync_delay"],
            mjpg_streamer=self.__config["video"]["mjpg_streamer"],
            loop=self.__loop,
        )

        self.__system_tasks: List[asyncio.Task] = []

    def run(self) -> None:
        app = aiohttp.web.Application(loop=self.__loop)
        app.router.add_get("/", self.__root_handler)
        app.router.add_get("/ws", self.__ws_handler)
        app.on_shutdown.append(self.__on_shutdown)
        app.on_cleanup.append(self.__on_cleanup)

        self.__system_tasks.extend([
            self.__loop.create_task(self.__poll_dead_sockets()),
            self.__loop.create_task(self.__poll_atx_leds()),
        ])

        aiohttp.web.run_app(
            app=app,
            host=self.__config["server"]["host"],
            port=self.__config["server"]["port"],
            print=(lambda text: [get_logger().info(line.strip()) for line in text.strip().splitlines()]),
        )

    async def __root_handler(self, _: aiohttp.web.Request) -> aiohttp.web.Response:
        return aiohttp.web.Response(text="OK")

    async def __ws_handler(self, request: aiohttp.web.Request) -> aiohttp.web.WebSocketResponse:
        ws = aiohttp.web.WebSocketResponse(**self.__config["ws"])
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

    async def __on_shutdown(self, _: aiohttp.web.Application) -> None:
        get_logger().info("Shutting down ...")
        for ws in list(self.__sockets):
            await self.__remove_socket(ws)

    async def __on_cleanup(self, _: aiohttp.web.Application) -> None:
        logger = get_logger()

        logger.info("Cancelling tasks ...")
        for task in self.__system_tasks:
            task.cancel()
        await asyncio.gather(*self.__system_tasks)

        logger.info("Cleaning up GPIO ...")
        GPIO.cleanup()

        logger.info("Bye-bye")

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
            await asyncio.sleep(self.__config["atx"]["leds"]["poll"])

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
        get_logger().warning("Received incorrect command: %r", command)
        return "ERROR incorrect command"

    async def __register_socket(self, ws: aiohttp.web.WebSocketResponse) -> None:
        async with self.__sockets_lock:
            self.__sockets.add(ws)
            get_logger().info("Registered new client socket: remote=%s; id=%d; active=%d",
                              ws._req.remote, id(ws), len(self.__sockets))  # pylint: disable=protected-access
            if len(self.__sockets) == 1:
                await self.__streamer.start()

    async def __remove_socket(self, ws: aiohttp.web.WebSocketResponse) -> None:
        async with self.__sockets_lock:
            try:
                self.__sockets.remove(ws)
                get_logger().info("Removed client socket: remote=%s; id=%d; active=%d",
                                  ws._req.remote, id(ws), len(self.__sockets))  # pylint: disable=protected-access
                await ws.close()
            except Exception:
                pass
            if not self.__sockets:
                await self.__streamer.stop()


def main() -> None:
    patch_logging()
    patch_threading()
    get_logger(app="kvmd")

    parser = argparse.ArgumentParser()
    parser.add_argument("-c", "--config", default="kvmd.yaml", metavar="<path>")
    options = parser.parse_args()

    with open(options.config) as config_file:
        config = yaml.load(config_file)
    logging.captureWarnings(True)
    logging.config.dictConfig(config["logging"])

    _Application(config["kvmd"]).run()
