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
import asyncio
import json

from enum import Enum

from typing import List
from typing import Dict
from typing import Set
from typing import Callable
from typing import AsyncGenerator
from typing import Optional
from typing import Any

import aiohttp
import aiohttp.web
import setproctitle

from ...logging import get_logger

from ...errors import OperationError
from ...errors import IsBusyError

from ...plugins import BasePlugin

from ...plugins.hid import BaseHid
from ...plugins.atx import BaseAtx
from ...plugins.msd import BaseMsd

from ...validators import ValidatorError

from ...validators.auth import valid_user
from ...validators.auth import valid_passwd
from ...validators.auth import valid_auth_token

from ...validators.kvm import valid_stream_quality
from ...validators.kvm import valid_stream_fps

from ... import aiotools

from ... import __version__

from .auth import AuthManager
from .info import InfoManager
from .logreader import LogReader
from .streamer import Streamer
from .wol import WakeOnLan

from .http import UnauthorizedError
from .http import ForbiddenError
from .http import HttpExposed
from .http import exposed_http
from .http import exposed_ws
from .http import get_exposed_http
from .http import get_exposed_ws
from .http import make_json_response
from .http import make_json_exception
from .http import set_request_auth_info
from .http import HttpServer

from .api.log import LogApi
from .api.wol import WolApi
from .api.hid import HidApi
from .api.atx import AtxApi
from .api.msd import MsdApi


# =====
_HEADER_AUTH_USER = "X-KVMD-User"
_HEADER_AUTH_PASSWD = "X-KVMD-Passwd"

_COOKIE_AUTH_TOKEN = "auth_token"


class _Events(Enum):
    INFO_STATE = "info_state"
    WOL_STATE = "wol_state"
    HID_STATE = "hid_state"
    ATX_STATE = "atx_state"
    MSD_STATE = "msd_state"
    STREAMER_STATE = "streamer_state"


class KvmdServer(HttpServer):  # pylint: disable=too-many-arguments,too-many-instance-attributes
    def __init__(  # pylint: disable=too-many-arguments
        self,
        auth_manager: AuthManager,
        info_manager: InfoManager,
        log_reader: LogReader,
        wol: WakeOnLan,

        hid: BaseHid,
        atx: BaseAtx,
        msd: BaseMsd,
        streamer: Streamer,

        heartbeat: float,
        sync_chunk_size: int,

        keymap_path: str,
    ) -> None:

        self.__auth_manager = auth_manager
        self.__info_manager = info_manager
        self.__wol = wol

        self.__hid = hid
        self.__atx = atx
        self.__msd = msd
        self.__streamer = streamer

        self.__heartbeat = heartbeat

        self.__apis: List[object] = [
            self,
            LogApi(log_reader),
            WolApi(wol),
            HidApi(hid, keymap_path),
            AtxApi(atx),
            MsdApi(msd, sync_chunk_size),
        ]

        self.__ws_handlers: Dict[str, Callable] = {}

        self.__sockets: Set[aiohttp.web.WebSocketResponse] = set()
        self.__sockets_lock = asyncio.Lock()

        self.__system_tasks: List[asyncio.Task] = []

        self.__streamer_notifier = aiotools.AioNotifier()
        self.__reset_streamer = False
        self.__new_streamer_params: Dict = {}

    async def __make_info(self) -> Dict:
        streamer_info = await self.__streamer.get_info()
        return {
            "version": {
                "kvmd": __version__,
                "streamer": streamer_info["version"],
            },
            "streamer": streamer_info["app"],
            "meta": await self.__info_manager.get_meta(),
            "extras": await self.__info_manager.get_extras(),
        }

    # ===== AUTH

    @exposed_http("POST", "/auth/login", auth_required=False)
    async def __auth_login_handler(self, request: aiohttp.web.Request) -> aiohttp.web.Response:
        if self.__auth_manager.is_auth_enabled():
            credentials = await request.post()
            token = await self.__auth_manager.login(
                user=valid_user(credentials.get("user", "")),
                passwd=valid_passwd(credentials.get("passwd", "")),
            )
            if token:
                return make_json_response(set_cookies={_COOKIE_AUTH_TOKEN: token})
            raise ForbiddenError()
        return make_json_response()

    @exposed_http("POST", "/auth/logout")
    async def __auth_logout_handler(self, request: aiohttp.web.Request) -> aiohttp.web.Response:
        if self.__auth_manager.is_auth_enabled():
            token = valid_auth_token(request.cookies.get(_COOKIE_AUTH_TOKEN, ""))
            self.__auth_manager.logout(token)
        return make_json_response()

    @exposed_http("GET", "/auth/check")
    async def __auth_check_handler(self, _: aiohttp.web.Request) -> aiohttp.web.Response:
        return make_json_response()

    # ===== SYSTEM

    @exposed_http("GET", "/info")
    async def __info_handler(self, _: aiohttp.web.Request) -> aiohttp.web.Response:
        return make_json_response(await self.__make_info())

    # ===== STREAMER

    @exposed_http("GET", "/streamer")
    async def __streamer_state_handler(self, _: aiohttp.web.Request) -> aiohttp.web.Response:
        return make_json_response(await self.__streamer.get_state())

    @exposed_http("POST", "/streamer/set_params")
    async def __streamer_set_params_handler(self, request: aiohttp.web.Request) -> aiohttp.web.Response:
        current_params = self.__streamer.get_params()
        for (name, validator) in [
            ("quality", valid_stream_quality),
            ("desired_fps", valid_stream_fps),
        ]:
            if (value := request.query.get(name)):
                value = validator(value)
                if current_params[name] != value:
                    self.__new_streamer_params[name] = value
        await self.__streamer_notifier.notify()
        return make_json_response()

    @exposed_http("POST", "/streamer/reset")
    async def __streamer_reset_handler(self, _: aiohttp.web.Request) -> aiohttp.web.Response:
        self.__reset_streamer = True
        await self.__streamer_notifier.notify()
        return make_json_response()

    # ===== WEBSOCKET

    @exposed_http("GET", "/ws")
    async def __ws_handler(self, request: aiohttp.web.Request) -> aiohttp.web.WebSocketResponse:
        logger = get_logger(0)
        ws = aiohttp.web.WebSocketResponse(heartbeat=self.__heartbeat)
        await ws.prepare(request)
        await self.__register_socket(ws)
        try:
            await asyncio.gather(*[
                self.__broadcast_event(_Events.INFO_STATE, (await self.__make_info())),
                self.__broadcast_event(_Events.WOL_STATE, self.__wol.get_state()),
                self.__broadcast_event(_Events.HID_STATE, (await self.__hid.get_state())),
                self.__broadcast_event(_Events.ATX_STATE, self.__atx.get_state()),
                self.__broadcast_event(_Events.MSD_STATE, (await self.__msd.get_state())),
                self.__broadcast_event(_Events.STREAMER_STATE, (await self.__streamer.get_state())),
            ])
            async for msg in ws:
                if msg.type == aiohttp.web.WSMsgType.TEXT:
                    try:
                        data = json.loads(msg.data)
                        event_type = data.get("event_type")
                        event = data["event"]
                    except Exception as err:
                        logger.error("Can't parse JSON event from websocket: %r", err)
                    else:
                        handler = self.__ws_handlers.get(event_type)
                        if handler:
                            await handler(ws, event)
                        else:
                            logger.error("Unknown websocket event: %r", data)
                else:
                    break
            return ws
        finally:
            await self.__remove_socket(ws)

    @exposed_ws("ping")
    async def __ws_ping_handler(self, ws: aiohttp.web.WebSocketResponse, _: Dict) -> None:
        await ws.send_str(json.dumps({"event_type": "pong", "event": {}}))

    # ===== SYSTEM STUFF

    def run(self, **kwargs: Any) -> None:  # type: ignore  # pylint: disable=arguments-differ
        self.__hid.start()
        setproctitle.setproctitle(f"kvmd/main: {setproctitle.getproctitle()}")
        super().run(**kwargs)

    async def _make_app(self) -> aiohttp.web.Application:
        app = aiohttp.web.Application()
        app.on_shutdown.append(self.__on_shutdown)
        app.on_cleanup.append(self.__on_cleanup)

        self.__run_system_task(self.__stream_controller)
        self.__run_system_task(self.__poll_state, _Events.HID_STATE, self.__hid.poll_state())
        self.__run_system_task(self.__poll_state, _Events.ATX_STATE, self.__atx.poll_state())
        self.__run_system_task(self.__poll_state, _Events.MSD_STATE, self.__msd.poll_state())
        self.__run_system_task(self.__poll_state, _Events.STREAMER_STATE, self.__streamer.poll_state())

        for api in self.__apis:
            for http_exposed in get_exposed_http(api):
                self.__add_app_route(app, http_exposed)
            for ws_exposed in get_exposed_ws(api):
                self.__ws_handlers[ws_exposed.event_type] = ws_exposed.handler

        return app

    def __run_system_task(self, method: Callable, *args: Any) -> None:
        async def wrapper() -> None:
            try:
                await method(*args)
                raise RuntimeError(f"Dead system task: {method.__name__}"
                                   f"({', '.join(getattr(arg, '__name__', str(arg)) for arg in args)})")
            except asyncio.CancelledError:
                pass
            except Exception:
                get_logger().exception("Unhandled exception, killing myself ...")
                os.kill(os.getpid(), signal.SIGTERM)
        self.__system_tasks.append(asyncio.create_task(wrapper()))

    def __add_app_route(self, app: aiohttp.web.Application, exposed: HttpExposed) -> None:
        async def wrapper(request: aiohttp.web.Request) -> aiohttp.web.Response:
            try:
                if exposed.auth_required and self.__auth_manager.is_auth_enabled():
                    user = request.headers.get(_HEADER_AUTH_USER, "")
                    passwd = request.headers.get(_HEADER_AUTH_PASSWD, "")
                    token = request.cookies.get(_COOKIE_AUTH_TOKEN, "")

                    if user:
                        user = valid_user(user)
                        set_request_auth_info(request, f"{user} (xhdr)")
                        if not (await self.__auth_manager.authorize(user, valid_passwd(passwd))):
                            raise ForbiddenError()

                    elif token:
                        user = self.__auth_manager.check(valid_auth_token(token))
                        if not user:
                            set_request_auth_info(request, "- (token)")
                            raise ForbiddenError()
                        set_request_auth_info(request, f"{user} (token)")

                    else:
                        raise UnauthorizedError()

                return (await exposed.handler(request))

            except IsBusyError as err:
                return make_json_exception(err, 409)
            except (ValidatorError, OperationError) as err:
                return make_json_exception(err, 400)
            except UnauthorizedError as err:
                return make_json_exception(err, 401)
            except ForbiddenError as err:
                return make_json_exception(err, 403)

        app.router.add_route(exposed.method, exposed.path, wrapper)

    async def __on_shutdown(self, _: aiohttp.web.Application) -> None:
        logger = get_logger(0)

        logger.info("Waiting short tasks ...")
        await asyncio.gather(*aiotools.get_short_tasks(), return_exceptions=True)

        logger.info("Cancelling system tasks ...")
        for task in self.__system_tasks:
            task.cancel()

        logger.info("Waiting system tasks ...")
        await asyncio.gather(*self.__system_tasks, return_exceptions=True)

        logger.info("Disconnecting clients ...")
        for ws in list(self.__sockets):
            await self.__remove_socket(ws)

    async def __on_cleanup(self, _: aiohttp.web.Application) -> None:
        logger = get_logger(0)
        for (name, obj) in [
            ("Auth manager", self.__auth_manager),
            ("Streamer", self.__streamer),
            ("MSD", self.__msd),
            ("ATX", self.__atx),
            ("HID", self.__hid),
        ]:
            if isinstance(obj, BasePlugin):
                name = f"{name} ({obj.get_plugin_name()})"
            logger.info("Cleaning up %s ...", name)
            try:
                await obj.cleanup()  # type: ignore
            except Exception:
                logger.exception("Cleanup error on %s", name)

    async def __broadcast_event(self, event_type: _Events, event: Dict) -> None:
        if self.__sockets:
            await asyncio.gather(*[
                ws.send_str(json.dumps({
                    "event_type": event_type.value,
                    "event": event,
                }))
                for ws in list(self.__sockets)
                if not ws.closed and ws._req is not None and ws._req.transport is not None  # pylint: disable=protected-access
            ], return_exceptions=True)

    async def __register_socket(self, ws: aiohttp.web.WebSocketResponse) -> None:
        async with self.__sockets_lock:
            self.__sockets.add(ws)
            remote: Optional[str] = (ws._req.remote if ws._req is not None else None)  # pylint: disable=protected-access
            get_logger().info("Registered new client socket: remote=%s; id=%d; active=%d", remote, id(ws), len(self.__sockets))
        await self.__streamer_notifier.notify()

    async def __remove_socket(self, ws: aiohttp.web.WebSocketResponse) -> None:
        async with self.__sockets_lock:
            self.__hid.clear_events()
            try:
                self.__sockets.remove(ws)
                remote: Optional[str] = (ws._req.remote if ws._req is not None else None)  # pylint: disable=protected-access
                get_logger().info("Removed client socket: remote=%s; id=%d; active=%d", remote, id(ws), len(self.__sockets))
                await ws.close()
            except Exception:
                pass
        await self.__streamer_notifier.notify()

    # ===== SYSTEM TASKS

    async def __stream_controller(self) -> None:
        prev = False
        while True:
            cur = bool(self.__sockets)
            if not prev and cur:
                await self.__streamer.ensure_start(init_restart=True)
            elif prev and not cur:
                await self.__streamer.ensure_stop(immediately=False)

            if self.__reset_streamer or self.__new_streamer_params:
                start = self.__streamer.is_working()
                await self.__streamer.ensure_stop(immediately=True)
                if self.__new_streamer_params:
                    self.__streamer.set_params(self.__new_streamer_params)
                    self.__new_streamer_params = {}
                if start:
                    await self.__streamer.ensure_start(init_restart=False)
                self.__reset_streamer = False

            prev = cur
            await self.__streamer_notifier.wait()

    async def __poll_state(self, event_type: _Events, poller: AsyncGenerator[Dict, None]) -> None:
        async for state in poller:
            await self.__broadcast_event(event_type, state)
