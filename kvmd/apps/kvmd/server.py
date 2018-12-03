import os
import signal
import asyncio
import json
import time

from enum import Enum

from typing import List
from typing import Dict
from typing import Set
from typing import Callable
from typing import Optional

import aiohttp.web
import setproctitle

from ...logging import get_logger

from ...aioregion import RegionIsBusyError

from ... import __version__

from .info import InfoManager
from .logreader import LogReader
from .hid import Hid
from .atx import Atx
from .msd import MsdOperationError
from .msd import MassStorageDevice
from .streamer import Streamer


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
    return aiohttp.web.Response(
        text=json.dumps({
            "ok": (True if status == 200 else False),
            "result": (result or {}),
        }, sort_keys=True, indent=4),
        status=status,
        content_type="application/json",
    )


def _json_exception(msg: str, err: Exception, status: int) -> aiohttp.web.Response:
    msg = "%s: %s" % (msg, err)
    get_logger().error(msg)
    return _json({
        "error": type(err).__name__,
        "error_msg": msg,
    }, status=status)


class BadRequest(Exception):
    pass


def _valid_bool(name: str, flag: Optional[str]) -> bool:
    flag = str(flag).strip().lower()
    if flag in ["1", "true", "yes"]:
        return True
    elif flag in ["0", "false", "no"]:
        return False
    raise BadRequest("Invalid param '%s'" % (name))


def _valid_int(name: str, value: Optional[str], min_value: Optional[int]=None, max_value: Optional[int]=None) -> int:
    try:
        value_int = int(value)  # type: ignore
        if (
            (min_value is not None and value_int < min_value)
            or (max_value is not None and value_int > max_value)
        ):
            raise ValueError()
        return value_int
    except Exception:
        raise BadRequest("Invalid param %r" % (name))


def _wrap_exceptions_for_web(msg: str) -> Callable:
    def make_wrapper(method: Callable) -> Callable:
        async def wrap(self: "Server", request: aiohttp.web.Request) -> aiohttp.web.Response:
            try:
                return (await method(self, request))
            except RegionIsBusyError as err:
                return _json_exception(msg, err, 409)
            except (BadRequest, MsdOperationError) as err:
                return _json_exception(msg, err, 400)
        return wrap
    return make_wrapper


class _Events(Enum):
    INFO_STATE = "info_state"
    ATX_STATE = "atx_state"
    MSD_STATE = "msd_state"
    STREAMER_STATE = "streamer_state"


class Server:  # pylint: disable=too-many-instance-attributes
    def __init__(  # pylint: disable=too-many-arguments
        self,
        info_manager: InfoManager,
        log_reader: LogReader,

        hid: Hid,
        atx: Atx,
        msd: MassStorageDevice,
        streamer: Streamer,

        heartbeat: float,
        streamer_shutdown_delay: float,
        msd_chunk_size: int,

        loop: asyncio.AbstractEventLoop,
    ) -> None:

        self.__info_manager = info_manager
        self.__log_reader = log_reader

        self.__hid = hid
        self.__atx = atx
        self.__msd = msd
        self.__streamer = streamer

        self.__heartbeat = heartbeat
        self.__streamer_shutdown_delay = streamer_shutdown_delay
        self.__msd_chunk_size = msd_chunk_size

        self.__loop = loop

        self.__sockets: Set[aiohttp.web.WebSocketResponse] = set()
        self.__sockets_lock = asyncio.Lock()

        self.__system_tasks: List[asyncio.Task] = []

        self.__reset_streamer = False
        self.__streamer_params = streamer.get_params()

    def run(self, host: str, port: int) -> None:
        self.__hid.start()

        setproctitle.setproctitle("[main] " + setproctitle.getproctitle())

        app = aiohttp.web.Application(loop=self.__loop)

        app.router.add_get("/info", self.__info_handler)
        app.router.add_get("/log", self.__log_handler)

        app.router.add_get("/ws", self.__ws_handler)

        app.router.add_post("/hid/reset", self.__hid_reset_handler)

        app.router.add_get("/atx", self.__atx_state_handler)
        app.router.add_post("/atx/click", self.__atx_click_handler)

        app.router.add_get("/msd", self.__msd_state_handler)
        app.router.add_post("/msd/connect", self.__msd_connect_handler)
        app.router.add_post("/msd/write", self.__msd_write_handler)
        app.router.add_post("/msd/reset", self.__msd_reset_handler)

        app.router.add_get("/streamer", self.__streamer_state_handler)
        app.router.add_post("/streamer/set_params", self.__streamer_set_params_handler)
        app.router.add_post("/streamer/reset", self.__streamer_reset_handler)

        app.on_shutdown.append(self.__on_shutdown)
        app.on_cleanup.append(self.__on_cleanup)

        self.__system_tasks.extend([
            self.__loop.create_task(self.__hid_watchdog()),
            self.__loop.create_task(self.__stream_controller()),
            self.__loop.create_task(self.__poll_dead_sockets()),
            self.__loop.create_task(self.__poll_atx_state()),
            self.__loop.create_task(self.__poll_msd_state()),
            self.__loop.create_task(self.__poll_streamer_state()),
        ])

        aiohttp.web.run_app(app, host=host, port=port, print=self.__run_app_print)

    async def __make_info(self) -> Dict:
        return {
            "version": {
                "kvmd": __version__,
                "streamer": await self.__streamer.get_version(),
            },
            "streamer": self.__streamer.get_app(),
            "meta": await self.__info_manager.get_meta(),
            "extras": await self.__info_manager.get_extras(),
        }

    # ===== SYSTEM

    async def __info_handler(self, _: aiohttp.web.Request) -> aiohttp.web.Response:
        return _json(await self.__make_info())

    @_wrap_exceptions_for_web("Log error")
    async def __log_handler(self, request: aiohttp.web.Request) -> aiohttp.web.StreamResponse:
        seek = _valid_int("seek", request.query.get("seek", "0"), 0)
        follow = _valid_bool("follow", request.query.get("follow", "false"))
        response = aiohttp.web.StreamResponse(status=200, reason="OK", headers={"Content-Type": "text/plain"})
        await response.prepare(request)
        async for record in self.__log_reader.poll_log(seek, follow):
            await response.write(("[%s %s] --- %s" % (
                record["dt"].strftime("%Y-%m-%d %H:%M:%S"),
                record["service"],
                record["msg"],
            )).encode("utf-8") + b"\r\n")
        return response

    # ===== WEBSOCKET

    async def __ws_handler(self, request: aiohttp.web.Request) -> aiohttp.web.WebSocketResponse:
        logger = get_logger(0)
        ws = aiohttp.web.WebSocketResponse(heartbeat=self.__heartbeat)
        await ws.prepare(request)
        await self.__register_socket(ws)
        await asyncio.gather(*[
            self.__broadcast_event(_Events.INFO_STATE, (await self.__make_info())),
            self.__broadcast_event(_Events.ATX_STATE, self.__atx.get_state()),
            self.__broadcast_event(_Events.MSD_STATE, self.__msd.get_state()),
            self.__broadcast_event(_Events.STREAMER_STATE, (await self.__streamer.get_state())),
        ])
        async for msg in ws:
            if msg.type == aiohttp.web.WSMsgType.TEXT:
                try:
                    event = json.loads(msg.data)
                except Exception as err:
                    logger.error("Can't parse JSON event from websocket: %s", err)
                else:
                    event_type = event.get("event_type")
                    if event_type == "ping":
                        await ws.send_str(json.dumps({"msg_type": "pong"}))
                    elif event_type == "key":
                        await self.__handle_ws_key_event(event)
                    elif event_type == "mouse_move":
                        await self.__handle_ws_mouse_move_event(event)
                    elif event_type == "mouse_button":
                        await self.__handle_ws_mouse_button_event(event)
                    elif event_type == "mouse_wheel":
                        await self.__handle_ws_mouse_wheel_event(event)
                    else:
                        logger.error("Unknown websocket event: %r", event)
            else:
                break
        return ws

    async def __handle_ws_key_event(self, event: Dict) -> None:
        key = str(event.get("key", ""))[:64].strip()
        state = event.get("state")
        if key and state in [True, False]:
            await self.__hid.send_key_event(key, state)

    async def __handle_ws_mouse_move_event(self, event: Dict) -> None:
        try:
            to_x = int(event["to"]["x"])
            to_y = int(event["to"]["y"])
        except Exception:
            return
        await self.__hid.send_mouse_move_event(to_x, to_y)

    async def __handle_ws_mouse_button_event(self, event: Dict) -> None:
        button = str(event.get("button", ""))[:64].strip()
        state = event.get("state")
        if button and state in [True, False]:
            await self.__hid.send_mouse_button_event(button, state)

    async def __handle_ws_mouse_wheel_event(self, event: Dict) -> None:
        try:
            delta_y = int(event["delta"]["y"])
        except Exception:
            return
        await self.__hid.send_mouse_wheel_event(delta_y)

    # ===== HID

    async def __hid_reset_handler(self, _: aiohttp.web.Request) -> aiohttp.web.Response:
        await self.__hid.reset()
        return _json()

    # ===== ATX

    async def __atx_state_handler(self, _: aiohttp.web.Request) -> aiohttp.web.Response:
        return _json(self.__atx.get_state())

    @_wrap_exceptions_for_web("Click error")
    async def __atx_click_handler(self, request: aiohttp.web.Request) -> aiohttp.web.Response:
        button = request.query.get("button")
        clicker = {
            "power": self.__atx.click_power,
            "power_long": self.__atx.click_power_long,
            "reset": self.__atx.click_reset,
        }.get(button)
        if not clicker:
            raise BadRequest("Invalid param 'button'")
        await clicker()
        return _json({"clicked": button})

    # ===== MSD

    async def __msd_state_handler(self, _: aiohttp.web.Request) -> aiohttp.web.Response:
        return _json(self.__msd.get_state())

    @_wrap_exceptions_for_web("Mass-storage error")
    async def __msd_connect_handler(self, request: aiohttp.web.Request) -> aiohttp.web.Response:
        to = request.query.get("to")
        if to == "kvm":
            return _json(await self.__msd.connect_to_kvm())
        elif to == "server":
            return _json(await self.__msd.connect_to_pc())
        else:
            raise BadRequest("Invalid param 'to'")

    @_wrap_exceptions_for_web("Can't write data to mass-storage device")
    async def __msd_write_handler(self, request: aiohttp.web.Request) -> aiohttp.web.Response:
        logger = get_logger(0)
        reader = await request.multipart()
        written = 0
        try:
            async with self.__msd:
                field = await reader.next()
                if not field or field.name != "image_name":
                    raise BadRequest("Missing 'image_name' field")
                image_name = (await field.read()).decode("utf-8")[:256]

                field = await reader.next()
                if not field or field.name != "image_data":
                    raise BadRequest("Missing 'image_data' field")

                logger.info("Writing image %r to mass-storage device ...", image_name)
                await self.__msd.write_image_info(image_name, False)
                while True:
                    chunk = await field.read_chunk(self.__msd_chunk_size)
                    if not chunk:
                        break
                    written = await self.__msd.write_image_chunk(chunk)
                await self.__msd.write_image_info(image_name, True)
        finally:
            if written != 0:
                logger.info("Written %d bytes to mass-storage device", written)
        return _json({"written": written})

    @_wrap_exceptions_for_web("Mass-storage error")
    async def __msd_reset_handler(self, _: aiohttp.web.Request) -> aiohttp.web.Response:
        await self.__msd.reset()
        return _json()

    # ===== STREAMER

    async def __streamer_state_handler(self, _: aiohttp.web.Request) -> aiohttp.web.Response:
        return _json(await self.__streamer.get_state())

    @_wrap_exceptions_for_web("Can't set stream params")
    async def __streamer_set_params_handler(self, request: aiohttp.web.Request) -> aiohttp.web.Response:
        for (name, validator) in [
            ("quality", lambda arg: _valid_int("quality", arg, 1, 100)),
            ("desired_fps", lambda arg: _valid_int("desired_fps", arg, 0, 30)),
        ]:
            value = request.query.get(name)
            if value:
                self.__streamer_params[name] = validator(value)
        return _json()

    async def __streamer_reset_handler(self, _: aiohttp.web.Request) -> aiohttp.web.Response:
        self.__reset_streamer = True
        return _json()

    # =====

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
        await self.__streamer.cleanup()
        await self.__msd.cleanup()
        await self.__hid.cleanup()

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
                    await self.__streamer.start(self.__streamer_params)
            elif prev > 0 and cur == 0:
                shutdown_at = time.time() + self.__streamer_shutdown_delay
            elif prev == 0 and cur == 0 and time.time() > shutdown_at:
                if self.__streamer.is_running():
                    await self.__streamer.stop()

            if (self.__reset_streamer or self.__streamer_params != self.__streamer.get_params()):
                if self.__streamer.is_running():
                    await self.__streamer.stop()
                    await self.__streamer.start(self.__streamer_params, no_init_restart=True)
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
        async for state in self.__atx.poll_state():
            await self.__broadcast_event(_Events.ATX_STATE, state)

    @_system_task
    async def __poll_msd_state(self) -> None:
        async for state in self.__msd.poll_state():
            await self.__broadcast_event(_Events.MSD_STATE, state)

    @_system_task
    async def __poll_streamer_state(self) -> None:
        async for state in self.__streamer.poll_state():
            await self.__broadcast_event(_Events.STREAMER_STATE, state)

    async def __broadcast_event(self, event_type: _Events, event_attrs: Dict) -> None:
        if self.__sockets:
            await asyncio.gather(*[
                ws.send_str(json.dumps({
                    "msg_type": "event",
                    "msg": {
                        "event": event_type.value,
                        "event_attrs": event_attrs,
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
