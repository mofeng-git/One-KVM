# ========================================================================== #
#                                                                            #
#    KVMD - The main Pi-KVM daemon.                                          #
#                                                                            #
#    Copyright (C) 2018  Maxim Devaev <mdevaev@gmail.com>                    #
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
import signal
import socket
import asyncio
import inspect
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

from ...validators import ValidatorError

from ...validators.basic import valid_bool
from ...validators.auth import valid_user
from ...validators.auth import valid_passwd
from ...validators.auth import valid_auth_token
from ...validators.kvm import valid_atx_button
from ...validators.kvm import valid_kvm_target
from ...validators.kvm import valid_log_seek
from ...validators.kvm import valid_stream_quality
from ...validators.kvm import valid_stream_fps

from ... import __version__

from .auth import AuthManager
from .info import InfoManager
from .logreader import LogReader
from .hid import Hid
from .atx import AtxOperationError
from .atx import Atx
from .msd import MsdOperationError
from .msd import MassStorageDevice
from .streamer import Streamer


# =====
try:
    from aiohttp.web import AccessLogger  # type: ignore  # pylint: disable=ungrouped-imports
except ImportError:
    from aiohttp.helpers import AccessLogger  # type: ignore  # pylint: disable=ungrouped-imports


_ATTR_KVMD_USER = "kvmd_user"


def _format_P(request: aiohttp.web.BaseRequest, *_, **__) -> str:  # type: ignore  # pylint: disable=invalid-name
    return (getattr(request, _ATTR_KVMD_USER, None) or "-")


AccessLogger._format_P = staticmethod(_format_P)  # type: ignore  # pylint: disable=protected-access


# =====
class HttpError(Exception):
    pass


class UnauthorizedError(HttpError):
    pass


class ForbiddenError(HttpError):
    pass


def _json(
    result: Optional[Dict]=None,
    status: int=200,
    set_cookies: Optional[Dict[str, str]]=None,
) -> aiohttp.web.Response:

    response = aiohttp.web.Response(
        text=json.dumps({
            "ok": (status == 200),
            "result": (result or {}),
        }, sort_keys=True, indent=4),
        status=status,
        content_type="application/json",
    )
    if set_cookies:
        for (key, value) in set_cookies.items():
            response.set_cookie(key, value)
    return response


def _json_exception(err: Exception, status: int) -> aiohttp.web.Response:
    name = type(err).__name__
    msg = str(err)
    if not isinstance(err, (UnauthorizedError, ForbiddenError)):
        get_logger().error("API error: %s: %s", name, msg)
    return _json({
        "error": name,
        "error_msg": msg,
    }, status=status)


_ATTR_EXPOSED = "exposed"
_ATTR_EXPOSED_METHOD = "exposed_method"
_ATTR_EXPOSED_PATH = "exposed_path"
_ATTR_SYSTEM_TASK = "system_task"

_COOKIE_AUTH_TOKEN = "auth_token"


def _exposed(http_method: str, path: str, auth_required: bool=True) -> Callable:
    def make_wrapper(method: Callable) -> Callable:
        async def wrap(self: "Server", request: aiohttp.web.Request) -> aiohttp.web.Response:
            try:
                if auth_required:
                    token = request.cookies.get(_COOKIE_AUTH_TOKEN, "")
                    if token:
                        user = self._auth_manager.check(valid_auth_token(token))
                        if not user:
                            raise ForbiddenError("Forbidden")
                        setattr(request, _ATTR_KVMD_USER, user)
                    else:
                        raise UnauthorizedError("Unauthorized")

                return (await method(self, request))

            except RegionIsBusyError as err:
                return _json_exception(err, 409)
            except (ValidatorError, AtxOperationError, MsdOperationError) as err:
                return _json_exception(err, 400)
            except UnauthorizedError as err:
                return _json_exception(err, 401)
            except ForbiddenError as err:
                return _json_exception(err, 403)

        setattr(wrap, _ATTR_EXPOSED, True)
        setattr(wrap, _ATTR_EXPOSED_METHOD, http_method)
        setattr(wrap, _ATTR_EXPOSED_PATH, path)
        return wrap
    return make_wrapper


def _system_task(method: Callable) -> Callable:
    async def wrap(self: "Server") -> None:
        try:
            await method(self)
            raise RuntimeError("Dead system task: %s" % (method))
        except asyncio.CancelledError:
            pass
        except Exception:
            get_logger().exception("Unhandled exception, killing myself ...")
            os.kill(os.getpid(), signal.SIGTERM)

    setattr(wrap, _ATTR_SYSTEM_TASK, True)
    return wrap


class _Events(Enum):
    INFO_STATE = "info_state"
    HID_STATE = "hid_state"
    ATX_STATE = "atx_state"
    MSD_STATE = "msd_state"
    STREAMER_STATE = "streamer_state"


class Server:  # pylint: disable=too-many-instance-attributes
    def __init__(
        self,
        auth_manager: AuthManager,
        info_manager: InfoManager,
        log_reader: LogReader,

        hid: Hid,
        atx: Atx,
        msd: MassStorageDevice,
        streamer: Streamer,

        loop: asyncio.AbstractEventLoop,
    ) -> None:

        self._auth_manager = auth_manager
        self.__info_manager = info_manager
        self.__log_reader = log_reader

        self.__hid = hid
        self.__atx = atx
        self.__msd = msd
        self.__streamer = streamer

        self.__loop = loop

        self.__heartbeat: Optional[float] = None  # Assigned in run() for consistance
        self.__sockets: Set[aiohttp.web.WebSocketResponse] = set()
        self.__sockets_lock = asyncio.Lock()

        self.__system_tasks: List[asyncio.Task] = []

        self.__reset_streamer = False
        self.__streamer_params = streamer.get_params()

    def run(
        self,
        host: str,
        port: int,
        unix_path: str,
        unix_rm: bool,
        unix_mode: int,
        heartbeat: float,
        access_log_format: str,
    ) -> None:

        self.__hid.start()

        setproctitle.setproctitle("[main] " + setproctitle.getproctitle())

        self.__heartbeat = heartbeat
        app = aiohttp.web.Application(loop=self.__loop)
        app.on_shutdown.append(self.__on_shutdown)
        app.on_cleanup.append(self.__on_cleanup)

        for name in dir(self):
            method = getattr(self, name)
            if inspect.ismethod(method):
                if getattr(method, _ATTR_SYSTEM_TASK, False):
                    self.__system_tasks.append(self.__loop.create_task(method()))
                elif getattr(method, _ATTR_EXPOSED, False):
                    # router = app.router
                    router = getattr(app, "router")  # FIXME: Dirty hack to avoid pylint crash
                    router.add_route(
                        getattr(method, _ATTR_EXPOSED_METHOD),
                        getattr(method, _ATTR_EXPOSED_PATH),
                        method,
                    )

        assert port or unix_path
        if unix_path:
            socket_kwargs: Dict = {}
            if unix_rm and os.path.exists(unix_path):
                os.remove(unix_path)
            server_socket = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
            server_socket.bind(unix_path)
            if unix_mode:
                os.chmod(unix_path, unix_mode)
            socket_kwargs = {"sock": server_socket}
        else:
            socket_kwargs = {"host": host, "port": port}

        aiohttp.web.run_app(
            app=app,
            access_log_format=access_log_format,
            print=self.__run_app_print,
            **socket_kwargs,
        )

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

    # ===== AUTH

    @_exposed("POST", "/auth/login", auth_required=False)
    async def __auth_login_handler(self, request: aiohttp.web.Request) -> aiohttp.web.Response:
        credentials = await request.post()
        token = self._auth_manager.login(
            user=valid_user(credentials.get("user", "")),
            passwd=valid_passwd(credentials.get("passwd", "")),
        )
        if token:
            return _json({}, set_cookies={_COOKIE_AUTH_TOKEN: token})
        raise ForbiddenError("Forbidden")

    @_exposed("POST", "/auth/logout")
    async def __auth_logout_handler(self, request: aiohttp.web.Request) -> aiohttp.web.Response:
        token = valid_auth_token(request.cookies.get(_COOKIE_AUTH_TOKEN, ""))
        self._auth_manager.logout(token)
        return _json({})

    @_exposed("GET", "/auth/check")
    async def __auth_check_handler(self, _: aiohttp.web.Request) -> aiohttp.web.Response:
        return _json({})

    # ===== SYSTEM

    @_exposed("GET", "/info")
    async def __info_handler(self, _: aiohttp.web.Request) -> aiohttp.web.Response:
        return _json(await self.__make_info())

    @_exposed("GET", "/log")
    async def __log_handler(self, request: aiohttp.web.Request) -> aiohttp.web.StreamResponse:
        seek = valid_log_seek(request.query.get("seek", "0"))
        follow = valid_bool(request.query.get("follow", "false"))
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

    @_exposed("GET", "/ws")
    async def __ws_handler(self, request: aiohttp.web.Request) -> aiohttp.web.WebSocketResponse:
        logger = get_logger(0)
        assert self.__heartbeat is not None
        ws = aiohttp.web.WebSocketResponse(heartbeat=self.__heartbeat)
        await ws.prepare(request)
        await self.__register_socket(ws)
        await asyncio.gather(*[
            self.__broadcast_event(_Events.INFO_STATE, (await self.__make_info())),
            self.__broadcast_event(_Events.HID_STATE, self.__hid.get_state()),
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

    @_exposed("GET", "/hid")
    async def __hid_state_handler(self, _: aiohttp.web.Request) -> aiohttp.web.Response:
        return _json(self.__hid.get_state())

    @_exposed("POST", "/hid/reset")
    async def __hid_reset_handler(self, _: aiohttp.web.Request) -> aiohttp.web.Response:
        await self.__hid.reset()
        return _json()

    # ===== ATX

    @_exposed("GET", "/atx")
    async def __atx_state_handler(self, _: aiohttp.web.Request) -> aiohttp.web.Response:
        return _json(self.__atx.get_state())

    @_exposed("POST", "/atx/click")
    async def __atx_click_handler(self, request: aiohttp.web.Request) -> aiohttp.web.Response:
        button = valid_atx_button(request.query.get("button"))
        await ({
            "power": self.__atx.click_power,
            "power_long": self.__atx.click_power_long,
            "reset": self.__atx.click_reset,
        }[button])()
        return _json({"clicked": button})

    # ===== MSD

    @_exposed("GET", "/msd")
    async def __msd_state_handler(self, _: aiohttp.web.Request) -> aiohttp.web.Response:
        return _json(self.__msd.get_state())

    @_exposed("POST", "/msd/connect")
    async def __msd_connect_handler(self, request: aiohttp.web.Request) -> aiohttp.web.Response:
        to = valid_kvm_target(request.query.get("to"))
        return _json(await ({
            "kvm": self.__msd.connect_to_kvm,
            "server": self.__msd.connect_to_pc,
        }[to])())

    @_exposed("POST", "/msd/write")
    async def __msd_write_handler(self, request: aiohttp.web.Request) -> aiohttp.web.Response:
        logger = get_logger(0)
        reader = await request.multipart()
        written = 0
        try:
            async with self.__msd:
                field = await reader.next()
                if not field or field.name != "image_name":
                    raise ValidatorError("Missing 'image_name' field")
                image_name = (await field.read()).decode("utf-8")[:256]

                field = await reader.next()
                if not field or field.name != "image_data":
                    raise ValidatorError("Missing 'image_data' field")

                logger.info("Writing image %r to mass-storage device ...", image_name)
                await self.__msd.write_image_info(image_name, False)
                while True:
                    chunk = await field.read_chunk(self.__msd.chunk_size)
                    if not chunk:
                        break
                    written = await self.__msd.write_image_chunk(chunk)
                await self.__msd.write_image_info(image_name, True)
        finally:
            if written != 0:
                logger.info("Written %d bytes to mass-storage device", written)
        return _json({"written": written})

    @_exposed("POST", "/msd/reset")
    async def __msd_reset_handler(self, _: aiohttp.web.Request) -> aiohttp.web.Response:
        await self.__msd.reset()
        return _json()

    # ===== STREAMER

    @_exposed("GET", "/streamer")
    async def __streamer_state_handler(self, _: aiohttp.web.Request) -> aiohttp.web.Response:
        return _json(await self.__streamer.get_state())

    @_exposed("POST", "/streamer/set_params")
    async def __streamer_set_params_handler(self, request: aiohttp.web.Request) -> aiohttp.web.Response:
        for (name, validator) in [
            ("quality", valid_stream_quality),
            ("desired_fps", valid_stream_fps),
        ]:
            value = request.query.get(name)
            if value:
                self.__streamer_params[name] = validator(value)
        return _json()

    @_exposed("POST", "/streamer/reset")
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
                if not ws.closed and ws._req is not None and ws._req.transport is not None  # pylint: disable=protected-access
            ], return_exceptions=True)

    async def __register_socket(self, ws: aiohttp.web.WebSocketResponse) -> None:
        async with self.__sockets_lock:
            self.__sockets.add(ws)
            remote: Optional[str] = (ws._req.remote if ws._req is not None else None)  # pylint: disable=protected-access
            get_logger().info("Registered new client socket: remote=%s; id=%d; active=%d", remote, id(ws), len(self.__sockets))

    async def __remove_socket(self, ws: aiohttp.web.WebSocketResponse) -> None:
        async with self.__sockets_lock:
            await self.__hid.clear_events()
            try:
                self.__sockets.remove(ws)
                remote: Optional[str] = (ws._req.remote if ws._req is not None else None)  # pylint: disable=protected-access
                get_logger().info("Removed client socket: remote=%s; id=%d; active=%d", remote, id(ws), len(self.__sockets))
                await ws.close()
            except Exception:
                pass

    # ===== SYSTEM TASKS

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
                shutdown_at = time.time() + self.__streamer.shutdown_delay
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
                if ws.closed or ws._req is None or ws._req.transport is None:  # pylint: disable=protected-access
                    await self.__remove_socket(ws)
            await asyncio.sleep(0.1)

    @_system_task
    async def __poll_hid_state(self) -> None:
        async for state in self.__hid.poll_state():
            await self.__broadcast_event(_Events.HID_STATE, state)

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
