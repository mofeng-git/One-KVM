import os
import signal
import asyncio
import json
import time

from typing import List
from typing import Dict
from typing import Set
from typing import Callable
from typing import Optional

import aiohttp.web

from .aioregion import RegionIsBusyError

from .hid import Hid

from .atx import Atx

from .msd import MassStorageOperationError
from .msd import MassStorageDevice

from .streamer import Streamer

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


def _json(result: Optional[Dict]=None, status: int=200) -> aiohttp.web.Response:
    return aiohttp.web.json_response({
        "ok": (True if status == 200 else False),
        "result": (result or {}),
    }, status=status)


def _json_exception(msg: str, err: Exception, status: int) -> aiohttp.web.Response:
    get_logger().error("%s: %s", msg, err)
    return _json({
        "error": type(err).__name__,
        "error_msg": str(err),
    }, status=status)


class BadRequest(Exception):
    pass


class PerformingAnotherOperation(Exception):
    def __init__(self) -> None:
        super().__init__("Performing another operation, please try again later")


def _wrap_exceptions_for_web(msg: str) -> Callable:
    def make_wrapper(method: Callable) -> Callable:
        async def wrap(self: "Server", request: aiohttp.web.Request) -> aiohttp.web.Response:
            try:
                try:
                    return (await method(self, request))
                except RegionIsBusyError:
                    raise PerformingAnotherOperation()
            except (BadRequest, MassStorageOperationError) as err:
                return _json_exception(msg, err, 400)
            except PerformingAnotherOperation as err:
                return _json_exception(msg, err, 409)
        return wrap
    return make_wrapper


class Server:  # pylint: disable=too-many-instance-attributes
    def __init__(
        self,
        hid: Hid,
        atx: Atx,
        msd: MassStorageDevice,
        streamer: Streamer,

        heartbeat: float,
        atx_state_poll: float,
        streamer_shutdown_delay: float,
        msd_chunk_size: int,

        loop: asyncio.AbstractEventLoop,
    ) -> None:

        self.__hid = hid
        self.__atx = atx
        self.__msd = msd
        self.__streamer = streamer

        self.__heartbeat = heartbeat
        self.__streamer_shutdown_delay = streamer_shutdown_delay
        self.__atx_state_poll = atx_state_poll
        self.__msd_chunk_size = msd_chunk_size

        self.__loop = loop

        self.__sockets: Set[aiohttp.web.WebSocketResponse] = set()
        self.__sockets_lock = asyncio.Lock()

        self.__system_tasks: List[asyncio.Task] = []

        self.__reset_streamer = False

    def run(self, host: str, port: int) -> None:
        self.__hid.start()

        app = aiohttp.web.Application(loop=self.__loop)

        app.router.add_get("/ws", self.__ws_handler)

        app.router.add_get("/atx", self.__atx_state_handler)
        app.router.add_post("/atx/click", self.__atx_click_handler)

        app.router.add_get("/msd", self.__msd_state_handler)
        app.router.add_post("/msd/connect", self.__msd_connect_handler)
        app.router.add_post("/msd/write", self.__msd_write_handler)

        app.router.add_get("/streamer", self.__streamer_state_handler)
        app.router.add_post("/streamer/reset", self.__streamer_reset_handler)

        app.on_shutdown.append(self.__on_shutdown)
        app.on_cleanup.append(self.__on_cleanup)

        self.__system_tasks.extend([
            self.__loop.create_task(self.__hid_watchdog()),
            self.__loop.create_task(self.__stream_controller()),
            self.__loop.create_task(self.__poll_dead_sockets()),
            self.__loop.create_task(self.__poll_atx_state()),
        ])

        aiohttp.web.run_app(app, host=host, port=port, print=self.__run_app_print)

    async def __ws_handler(self, request: aiohttp.web.Request) -> aiohttp.web.WebSocketResponse:
        logger = get_logger(0)
        ws = aiohttp.web.WebSocketResponse(heartbeat=self.__heartbeat)
        await ws.prepare(request)
        await self.__register_socket(ws)
        async for msg in ws:
            if msg.type == aiohttp.web.WSMsgType.TEXT:
                try:
                    event = json.loads(msg.data)
                except Exception as err:
                    logger.error("Can't parse JSON event from websocket: %s", err)
                else:
                    if event.get("event_type") == "key":
                        key = str(event.get("key", ""))[:64].strip()
                        state = event.get("state")
                        if key and state in [True, False]:
                            await self.__hid.send_key_event(key, state)
                            continue
                    elif event.get("event_type") in ["mouse_move", "mouse_button", "mouse_wheel"]:
                        pass
                    else:
                        logger.error("Invalid websocket event: %r", event)
                await ws.send_str(json.dumps({"msg_type": "echo", "msg": msg.data}))
            else:
                break
        return ws

    async def __atx_state_handler(self, _: aiohttp.web.Request) -> aiohttp.web.Response:
        return _json(self.__atx.get_state())

    @_wrap_exceptions_for_web("Click error")
    async def __atx_click_handler(self, request: aiohttp.web.Request) -> aiohttp.web.Response:
        button = request.query.get("button")
        if button == "power":
            await self.__atx.click_power()
        elif button == "power_long":
            await self.__atx.click_power_long()
        elif button == "reset":
            await self.__atx.click_reset()
        else:
            raise BadRequest("Missing or invalid 'button=%s'" % (button))
        return _json({"clicked": button})

    async def __msd_state_handler(self, _: aiohttp.web.Request) -> aiohttp.web.Response:
        return _json(self.__msd.get_state())

    @_wrap_exceptions_for_web("Mass-storage error")
    async def __msd_connect_handler(self, request: aiohttp.web.Request) -> aiohttp.web.Response:
        to = request.query.get("to")
        if to == "kvm":
            await self.__msd.connect_to_kvm()
            await self.__broadcast_event("msd_state", state="connected_to_kvm")  # type: ignore
        elif to == "server":
            await self.__msd.connect_to_pc()
            await self.__broadcast_event("msd_state", state="connected_to_server")  # type: ignore
        else:
            raise BadRequest("Missing or invalid 'to=%s'" % (to))
        return _json(self.__msd.get_state())

    @_wrap_exceptions_for_web("Can't write data to mass-storage device")
    async def __msd_write_handler(self, request: aiohttp.web.Request) -> aiohttp.web.Response:
        logger = get_logger(0)
        reader = await request.multipart()
        written = 0
        try:
            field = await reader.next()
            if not field or field.name != "image_name":
                raise BadRequest("Missing 'image_name' field")
            image_name = (await field.read()).decode("utf-8")[:256]

            field = await reader.next()
            if not field or field.name != "image_data":
                raise BadRequest("Missing 'image_data' field")

            async with self.__msd:
                await self.__broadcast_event("msd_state", state="busy")  # type: ignore
                logger.info("Writing image %r to mass-storage device ...", image_name)
                await self.__msd.write_image_info(image_name, False)
                while True:
                    chunk = await field.read_chunk(self.__msd_chunk_size)
                    if not chunk:
                        break
                    written = await self.__msd.write_image_chunk(chunk)
                await self.__msd.write_image_info(image_name, True)
            await self.__broadcast_event("msd_state", state="free")  # type: ignore
        finally:
            if written != 0:
                logger.info("written %d bytes to mass-storage device", written)
        return _json({"written": written})

    async def __streamer_state_handler(self, _: aiohttp.web.Request) -> aiohttp.web.Response:
        return _json(self.__streamer.get_state())

    async def __streamer_reset_handler(self, _: aiohttp.web.Request) -> aiohttp.web.Response:
        self.__reset_streamer = True
        return _json()

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
        await self.__hid.cleanup()
        await self.__streamer.cleanup()
        await self.__msd.cleanup()

    @_system_task
    async def __hid_watchdog(self) -> None:
        while self.__hid.is_alive():
            await asyncio.sleep(0.1)
        raise RuntimeError("HID is dead")

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
                shutdown_at = time.time() + self.__streamer_shutdown_delay
            elif prev == 0 and cur == 0 and time.time() > shutdown_at:
                if self.__streamer.is_running():
                    await self.__streamer.stop()

            if self.__reset_streamer:
                if self.__streamer.is_running():
                    await self.__streamer.stop()
                    await self.__streamer.start()
                self.__reset_streamer = False

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
    async def __poll_atx_state(self) -> None:
        while True:
            if self.__sockets:
                await self.__broadcast_event("atx_state", **self.__atx.get_state())
            await asyncio.sleep(self.__atx_state_poll)

    async def __broadcast_event(self, event: str, **kwargs: Dict) -> None:
        await asyncio.gather(*[
            ws.send_str(json.dumps({
                "msg_type": "event",
                "msg": {
                    "event": event,
                    "event_attrs": kwargs,
                },
            }))
            for ws in list(self.__sockets)
            if not ws.closed and ws._req.transport  # pylint: disable=protected-access
        ], return_exceptions=True)

    async def __register_socket(self, ws: aiohttp.web.WebSocketResponse) -> None:
        async with self.__sockets_lock:
            self.__sockets.add(ws)
            get_logger().info("Registered new client socket: remote=%s; id=%d; active=%d",
                              ws._req.remote, id(ws), len(self.__sockets))  # pylint: disable=protected-access

    async def __remove_socket(self, ws: aiohttp.web.WebSocketResponse) -> None:
        async with self.__sockets_lock:
            await self.__hid.clear_events()
            try:
                self.__sockets.remove(ws)
                get_logger().info("Removed client socket: remote=%s; id=%d; active=%d",
                                  ws._req.remote, id(ws), len(self.__sockets))  # pylint: disable=protected-access
                await ws.close()
            except Exception:
                pass
