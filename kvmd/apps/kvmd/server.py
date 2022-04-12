# ========================================================================== #
#                                                                            #
#    KVMD - The main PiKVM daemon.                                           #
#                                                                            #
#    Copyright (C) 2018-2022  Maxim Devaev <mdevaev@gmail.com>               #
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

from typing import Tuple
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

from ... import aiotools
from ... import aioproc

from ...htserver import HttpExposed
from ...htserver import exposed_http
from ...htserver import exposed_ws
from ...htserver import get_exposed_http
from ...htserver import get_exposed_ws
from ...htserver import make_json_response
from ...htserver import send_ws_event
from ...htserver import broadcast_ws_event
from ...htserver import process_ws_messages
from ...htserver import HttpServer

from ...plugins import BasePlugin
from ...plugins.hid import BaseHid
from ...plugins.atx import BaseAtx
from ...plugins.msd import BaseMsd

from ...validators.basic import valid_bool
from ...validators.kvm import valid_stream_quality
from ...validators.kvm import valid_stream_fps
from ...validators.kvm import valid_stream_resolution
from ...validators.kvm import valid_stream_h264_bitrate
from ...validators.kvm import valid_stream_h264_gop

from .auth import AuthManager
from .info import InfoManager
from .logreader import LogReader
from .ugpio import UserGpio
from .streamer import Streamer
from .snapshoter import Snapshoter
from .tesseract import TesseractOcr

from .api.auth import AuthApi
from .api.auth import check_request_auth

from .api.info import InfoApi
from .api.log import LogApi
from .api.ugpio import UserGpioApi
from .api.hid import HidApi
from .api.atx import AtxApi
from .api.msd import MsdApi
from .api.streamer import StreamerApi
from .api.export import ExportApi
from .api.redfish import RedfishApi


# =====
class StreamerQualityNotSupported(OperationError):
    def __init__(self) -> None:
        super().__init__("This streamer does not support quality settings")


class StreamerResolutionNotSupported(OperationError):
    def __init__(self) -> None:
        super().__init__("This streamer does not support resolution settings")


class StreamerH264NotSupported(OperationError):
    def __init__(self) -> None:
        super().__init__("This streamer does not support H264")


# =====
@dataclasses.dataclass(frozen=True)
class _Component:  # pylint: disable=too-many-instance-attributes
    name: str
    event_type: str
    obj: object
    sysprep: Optional[Callable[[], None]] = None
    systask: Optional[Callable[[], Coroutine[Any, Any, None]]] = None
    get_state: Optional[Callable[[], Coroutine[Any, Any, Dict]]] = None
    poll_state: Optional[Callable[[], AsyncGenerator[Dict, None]]] = None
    cleanup: Optional[Callable[[], Coroutine[Any, Any, Dict]]] = None

    def __post_init__(self) -> None:
        if isinstance(self.obj, BasePlugin):
            object.__setattr__(self, "name", f"{self.name} ({self.obj.get_plugin_name()})")

        for field in ["sysprep", "systask", "get_state", "poll_state", "cleanup"]:
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
    def __init__(  # pylint: disable=too-many-arguments,too-many-locals
        self,
        auth_manager: AuthManager,
        info_manager: InfoManager,
        log_reader: LogReader,
        user_gpio: UserGpio,
        ocr: TesseractOcr,

        hid: BaseHid,
        atx: BaseAtx,
        msd: BaseMsd,
        streamer: Streamer,
        snapshoter: Snapshoter,

        keymap_path: str,
        ignore_keys: List[str],
        mouse_x_range: Tuple[int, int],
        mouse_y_range: Tuple[int, int],

        stream_forever: bool,
    ) -> None:

        self.__auth_manager = auth_manager
        self.__hid = hid
        self.__streamer = streamer
        self.__snapshoter = snapshoter  # Not a component: No state or cleanup
        self.__user_gpio = user_gpio  # Has extra state "gpio_scheme_state"

        self.__stream_forever = stream_forever

        self.__components = [
            *[
                _Component("Auth manager", "", auth_manager),
            ],
            *[
                _Component(f"Info manager ({sub})", f"info_{sub}_state", info_manager.get_submanager(sub))
                for sub in sorted(info_manager.get_subs())
            ],
            *[
                _Component("User-GPIO",    "gpio_state",     user_gpio),
                _Component("HID",          "hid_state",      hid),
                _Component("ATX",          "atx_state",      atx),
                _Component("MSD",          "msd_state",      msd),
                _Component("Streamer",     "streamer_state", streamer),
            ],
        ]

        self.__hid_api = HidApi(hid, keymap_path, ignore_keys, mouse_x_range, mouse_y_range)  # Ugly hack to get keymaps state
        self.__streamer_api = StreamerApi(streamer, ocr)  # Same hack to get ocr langs state
        self.__apis: List[object] = [
            self,
            AuthApi(auth_manager),
            InfoApi(info_manager),
            LogApi(log_reader),
            UserGpioApi(user_gpio),
            self.__hid_api,
            AtxApi(atx),
            MsdApi(msd),
            self.__streamer_api,
            ExportApi(info_manager, atx, user_gpio),
            RedfishApi(info_manager, atx),
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
        for (name, validator, exc_cls) in [
            ("quality", valid_stream_quality, StreamerQualityNotSupported),
            ("desired_fps", valid_stream_fps, None),
            ("resolution", valid_stream_resolution, StreamerResolutionNotSupported),
            ("h264_bitrate", valid_stream_h264_bitrate, StreamerH264NotSupported),
            ("h264_gop", valid_stream_h264_gop, StreamerH264NotSupported),
        ]:
            value = request.query.get(name)
            if value:
                if name not in current_params:
                    assert exc_cls is not None, name
                    raise exc_cls()
                value = validator(value)  # type: ignore
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
        stream = valid_bool(request.query.get("stream", "true"))
        ws = await self._make_ws_response(request)
        client = _WsClient(ws, stream)
        await self.__register_ws_client(client)

        try:
            stage1 = [
                ("gpio_model_state", self.__user_gpio.get_model()),
                ("hid_keymaps_state", self.__hid_api.get_keymaps()),
                ("streamer_ocr_state", self.__streamer_api.get_ocr()),
            ]
            stage2 = [
                (comp.event_type, comp.get_state())
                for comp in self.__components
                if comp.get_state
            ]
            stages = stage1 + stage2
            events = dict(zip(
                map(operator.itemgetter(0), stages),
                await asyncio.gather(*map(operator.itemgetter(1), stages)),
            ))
            for stage in [stage1, stage2]:
                await asyncio.gather(*[
                    send_ws_event(ws, event_type, events.pop(event_type))
                    for (event_type, _) in stage
                ])

            await send_ws_event(ws, "loop", {})
            await process_ws_messages(ws, self.__ws_handlers)
            return ws
        finally:
            await self.__remove_ws_client(client)

    @exposed_ws("ping")
    async def __ws_ping_handler(self, ws: aiohttp.web.WebSocketResponse, _: Dict) -> None:
        await send_ws_event(ws, "pong", {})

    # ===== SYSTEM STUFF

    def run(self, **kwargs: Any) -> None:  # type: ignore  # pylint: disable=arguments-differ
        for comp in self.__components:
            if comp.sysprep:
                comp.sysprep()
        aioproc.rename_process("main")
        super().run(**kwargs)

    async def _check_request_auth(self, exposed: HttpExposed, request: aiohttp.web.Request) -> None:
        await check_request_auth(self.__auth_manager, exposed, request)

    async def _init_app(self, _: aiohttp.web.Application) -> None:
        self.__run_system_task(self.__stream_controller)
        for comp in self.__components:
            if comp.systask:
                self.__run_system_task(comp.systask)
            if comp.poll_state:
                self.__run_system_task(self.__poll_state, comp.event_type, comp.poll_state())
        self.__run_system_task(self.__stream_snapshoter)

        for api in self.__apis:
            for http_exposed in get_exposed_http(api):
                self._add_exposed(http_exposed)
            for ws_exposed in get_exposed_ws(api):
                self.__ws_handlers[ws_exposed.event_type] = ws_exposed.handler

    def __run_system_task(self, method: Callable, *args: Any) -> None:
        async def wrapper() -> None:
            try:
                await method(*args)
                raise RuntimeError(f"Dead system task: {method}"
                                   f"({', '.join(getattr(arg, '__name__', str(arg)) for arg in args)})")
            except asyncio.CancelledError:
                pass
            except Exception:
                get_logger().exception("Unhandled exception, killing myself ...")
                os.kill(os.getpid(), signal.SIGTERM)
        self.__system_tasks.append(asyncio.create_task(wrapper()))

    async def _on_shutdown(self, _: aiohttp.web.Application) -> None:
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

        logger.info("On-Shutdown complete")

    async def _on_cleanup(self, _: aiohttp.web.Application) -> None:
        logger = get_logger(0)
        for comp in self.__components:
            if comp.cleanup:
                logger.info("Cleaning up %s ...", comp.name)
                try:
                    await comp.cleanup()  # type: ignore
                except Exception:
                    logger.exception("Cleanup error on %s", comp.name)
        logger.info("On-Cleanup complete")

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
            cur = (self.__has_stream_clients() or self.__snapshoter.snapshoting() or self.__stream_forever)
            if not prev and cur:
                await self.__streamer.ensure_start(reset=False)
            elif prev and not cur:
                await self.__streamer.ensure_stop(immediately=False)

            if self.__reset_streamer or self.__new_streamer_params:
                start = self.__streamer.is_working()
                await self.__streamer.ensure_stop(immediately=True)
                if self.__new_streamer_params:
                    self.__streamer.set_params(self.__new_streamer_params)
                    self.__new_streamer_params = {}
                if start:
                    await self.__streamer.ensure_start(reset=self.__reset_streamer)
                self.__reset_streamer = False

            prev = cur
            await self.__streamer_notifier.wait()

    async def __poll_state(self, event_type: str, poller: AsyncGenerator[Dict, None]) -> None:
        async for state in poller:
            await broadcast_ws_event([
                client.ws
                for client in list(self.__ws_clients)
            ], event_type, state)

    async def __stream_snapshoter(self) -> None:
        await self.__snapshoter.run(
            is_live=self.__has_stream_clients,
            notifier=self.__streamer_notifier,
        )
