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
import operator
import dataclasses
import json

from typing import List
from typing import Dict
from typing import Set
from typing import Callable
from typing import Coroutine
from typing import AsyncGenerator
from typing import Optional
from typing import Any

import aiohttp
import aiohttp.web

from ...logging import get_logger

from ...errors import OperationError
from ...errors import IsBusyError

from ...plugins import BasePlugin

from ...plugins.hid import BaseHid
from ...plugins.atx import BaseAtx
from ...plugins.msd import BaseMsd

from ...validators import ValidatorError

from ...validators.basic import valid_bool

from ...validators.kvm import valid_stream_quality
from ...validators.kvm import valid_stream_fps

from ... import aiotools
from ... import aioproc

from .auth import AuthManager
from .info import InfoManager
from .logreader import LogReader
from .wol import WakeOnLan
from .streamer import Streamer
from .snapshoter import Snapshoter

from .http import HttpError
from .http import HttpExposed
from .http import exposed_http
from .http import exposed_ws
from .http import get_exposed_http
from .http import get_exposed_ws
from .http import make_json_response
from .http import make_json_exception
from .http import HttpServer

from .api.auth import AuthApi
from .api.auth import check_request_auth

from .api.info import InfoApi
from .api.log import LogApi
from .api.wol import WolApi
from .api.hid import HidApi
from .api.atx import AtxApi
from .api.msd import MsdApi
from .api.streamer import StreamerApi


# =====
@dataclasses.dataclass(frozen=True)
class _Component:
    name: str
    event_type: str
    obj: object
    get_state: Optional[Callable[[], Coroutine[Any, Any, Dict]]] = None
    poll_state: Optional[Callable[[], AsyncGenerator[Dict, None]]] = None
    cleanup: Optional[Callable[[], Coroutine[Any, Any, Dict]]] = None

    def __post_init__(self) -> None:
        if isinstance(self.obj, BasePlugin):
            object.__setattr__(self, "name", f"{self.name} ({self.obj.get_plugin_name()})")

        for field in ["get_state", "poll_state", "cleanup"]:
            object.__setattr__(self, field, getattr(self.obj, field, None))
        if self.get_state or self.poll_state:
            assert self.event_type, self


@dataclasses.dataclass(frozen=True)
class _WsClient:
    ws: aiohttp.web.WebSocketResponse
    stream: bool

    def __str__(self) -> str:
        return f"WsClient(id={id(self)}, stream={self.stream})"


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
        snapshoter: Snapshoter,

        heartbeat: float,
        sync_chunk_size: int,

        keymap_path: str,
    ) -> None:

        self.__auth_manager = auth_manager
        self.__hid = hid
        self.__streamer = streamer
        self.__snapshoter = snapshoter  # Not a component: No state or cleanup

        self.__heartbeat = heartbeat

        self.__components = [
            _Component("Auth manager", "",               auth_manager),
            _Component("Info manager", "info_state",     info_manager),
            _Component("Wake-on-LAN",  "wol_state",      wol),
            _Component("HID",          "hid_state",      hid),
            _Component("ATX",          "atx_state",      atx),
            _Component("MSD",          "msd_state",      msd),
            _Component("Streamer",     "streamer_state", streamer),
        ]

        self.__apis: List[object] = [
            self,
            AuthApi(auth_manager),
            InfoApi(info_manager),
            LogApi(log_reader),
            WolApi(wol),
            HidApi(hid, keymap_path),
            AtxApi(atx),
            MsdApi(msd, sync_chunk_size),
            StreamerApi(streamer),
        ]

        self.__ws_handlers: Dict[str, Callable] = {}

        self.__ws_clients: Set[_WsClient] = set()
        self.__ws_clients_lock = asyncio.Lock()

        self.__system_tasks: List[asyncio.Task] = []

        self.__streamer_notifier = aiotools.AioNotifier()
        self.__reset_streamer = False
        self.__new_streamer_params: Dict = {}

    # ===== STREAMER CONTROLLER

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
        client = _WsClient(
            ws=aiohttp.web.WebSocketResponse(heartbeat=self.__heartbeat),
            stream=valid_bool(request.query.get("stream", "true")),
        )
        await client.ws.prepare(request)
        await self.__register_ws_client(client)
        try:
            await asyncio.gather(*[
                self.__broadcast_event(component.event_type, await component.get_state())
                for component in self.__components
                if component.get_state
            ])
            async for msg in client.ws:
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
                            await handler(client.ws, event)
                        else:
                            logger.error("Unknown websocket event: %r", data)
                else:
                    break
            return client.ws
        finally:
            await self.__remove_ws_client(client)

    @exposed_ws("ping")
    async def __ws_ping_handler(self, ws: aiohttp.web.WebSocketResponse, _: Dict) -> None:
        await ws.send_str(json.dumps({"event_type": "pong", "event": {}}))

    # ===== SYSTEM STUFF

    def run(self, **kwargs: Any) -> None:  # type: ignore  # pylint: disable=arguments-differ
        self.__hid.start()
        aioproc.rename_process("main")
        super().run(**kwargs)

    async def _make_app(self) -> aiohttp.web.Application:
        app = aiohttp.web.Application()
        app.on_shutdown.append(self.__on_shutdown)
        app.on_cleanup.append(self.__on_cleanup)

        self.__run_system_task(self.__stream_controller)
        for component in self.__components:
            if component.poll_state:
                self.__run_system_task(self.__poll_state, component.event_type, component.poll_state())
        self.__run_system_task(self.__stream_snapshoter)

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
                await check_request_auth(self.__auth_manager, exposed, request)
                return (await exposed.handler(request))
            except IsBusyError as err:
                return make_json_exception(err, 409)
            except (ValidatorError, OperationError) as err:
                return make_json_exception(err, 400)
            except HttpError as err:
                return make_json_exception(err)
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
        for client in list(self.__ws_clients):
            await self.__remove_ws_client(client)

    async def __on_cleanup(self, _: aiohttp.web.Application) -> None:
        logger = get_logger(0)
        for component in self.__components:
            if component.cleanup:
                logger.info("Cleaning up %s ...", component.name)
                try:
                    await component.cleanup()  # type: ignore
                except Exception:
                    logger.exception("Cleanup error on %s", component.name)

    async def __broadcast_event(self, event_type: str, event: Dict) -> None:
        if self.__ws_clients:
            await asyncio.gather(*[
                client.ws.send_str(json.dumps({
                    "event_type": event_type,
                    "event": event,
                }))
                for client in list(self.__ws_clients)
                if (
                    not client.ws.closed
                    and client.ws._req is not None  # pylint: disable=protected-access
                    and client.ws._req.transport is not None  # pylint: disable=protected-access
                )
            ], return_exceptions=True)

    async def __register_ws_client(self, client: _WsClient) -> None:
        async with self.__ws_clients_lock:
            self.__ws_clients.add(client)
            get_logger().info("Registered new client socket: %s; clients now: %d", client, len(self.__ws_clients))
        await self.__streamer_notifier.notify()

    async def __remove_ws_client(self, client: _WsClient) -> None:
        async with self.__ws_clients_lock:
            self.__hid.clear_events()
            try:
                self.__ws_clients.remove(client)
                get_logger().info("Removed client socket: %s; clients now: %d", client, len(self.__ws_clients))
                await client.ws.close()
            except Exception:
                pass
        await self.__streamer_notifier.notify()

    def __has_stream_clients(self) -> bool:
        return bool(sum(map(operator.attrgetter("stream"), self.__ws_clients)))

    # ===== SYSTEM TASKS

    async def __stream_controller(self) -> None:
        prev = False
        while True:
            cur = (self.__has_stream_clients() or self.__snapshoter.snapshoting())
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

    async def __poll_state(self, event_type: str, poller: AsyncGenerator[Dict, None]) -> None:
        async for state in poller:
            await self.__broadcast_event(event_type, state)

    async def __stream_snapshoter(self) -> None:
        await self.__snapshoter.run(
            is_live=self.__has_stream_clients,
            notifier=self.__streamer_notifier,
        )
