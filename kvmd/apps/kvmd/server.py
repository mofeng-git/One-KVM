# ========================================================================== #
#                                                                            #
#    KVMD - The main PiKVM daemon.                                           #
#                                                                            #
#    Copyright (C) 2018-2023  Maxim Devaev <mdevaev@gmail.com>               #
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
import operator
import dataclasses

from typing import Callable
from typing import Coroutine
from typing import AsyncGenerator
from typing import Any

from aiohttp.web import Request
from aiohttp.web import Response
from aiohttp.web import WebSocketResponse

from ...logging import get_logger

from ...errors import OperationError

from ... import aiotools
from ... import aioproc

from ...htserver import HttpExposed
from ...htserver import exposed_http
from ...htserver import exposed_ws
from ...htserver import make_json_response
from ...htserver import WsSession
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
from .ocr import Ocr

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
class _SubsystemEventSource:
    get_state:  (Callable[[], Coroutine[Any, Any, dict]] | None) = None
    poll_state: (Callable[[], AsyncGenerator[dict, None]] | None) = None


@dataclasses.dataclass
class _Subsystem:
    name:    str
    sysprep: (Callable[[], None] | None)
    systask: (Callable[[], Coroutine[Any, Any, None]] | None)
    cleanup: (Callable[[], Coroutine[Any, Any, dict]] | None)
    sources: dict[str, _SubsystemEventSource]

    @classmethod
    def make(cls, obj: object, name: str, event_type: str="") -> "_Subsystem":
        if isinstance(obj, BasePlugin):
            name = f"{name} ({obj.get_plugin_name()})"
        sub = _Subsystem(
            name=name,
            sysprep=getattr(obj, "sysprep", None),
            systask=getattr(obj, "systask", None),
            cleanup=getattr(obj, "cleanup", None),
            sources={},
        )
        if event_type:
            sub.add_source(
                event_type=event_type,
                get_state=getattr(obj, "get_state", None),
                poll_state=getattr(obj, "poll_state", None),
            )
        return sub

    def add_source(
        self,
        event_type: str,
        get_state: (Callable[[], Coroutine[Any, Any, dict]] | None),
        poll_state: (Callable[[], AsyncGenerator[dict, None]] | None),
    ) -> "_Subsystem":

        assert event_type
        assert event_type not in self.sources, (self, event_type)
        assert get_state or poll_state, (self, event_type)
        self.sources[event_type] = _SubsystemEventSource(get_state, poll_state)
        return self


class KvmdServer(HttpServer):  # pylint: disable=too-many-arguments,too-many-instance-attributes
    def __init__(  # pylint: disable=too-many-arguments,too-many-locals
        self,
        auth_manager: AuthManager,
        info_manager: InfoManager,
        log_reader: (LogReader | None),
        user_gpio: UserGpio,
        ocr: Ocr,

        hid: BaseHid,
        atx: BaseAtx,
        msd: BaseMsd,
        streamer: Streamer,
        snapshoter: Snapshoter,

        keymap_path: str,
        ignore_keys: list[str],
        mouse_x_range: tuple[int, int],
        mouse_y_range: tuple[int, int],

        stream_forever: bool,
    ) -> None:

        super().__init__()

        self.__auth_manager = auth_manager
        self.__hid = hid
        self.__streamer = streamer
        self.__snapshoter = snapshoter  # Not a component: No state or cleanup

        self.__stream_forever = stream_forever

        self.__hid_api = HidApi(hid, keymap_path, ignore_keys, mouse_x_range, mouse_y_range)  # Ugly hack to get keymaps state
        self.__streamer_api = StreamerApi(streamer, ocr)  # Same hack to get ocr langs state
        self.__apis: list[object] = [
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

        self.__subsystems = [
            _Subsystem.make(auth_manager, "Auth manager"),
            _Subsystem.make(user_gpio,    "User-GPIO", "gpio_state").add_source("gpio_model_state", user_gpio.get_model, None),
            _Subsystem.make(hid,          "HID",       "hid_state").add_source("hid_keymaps_state", self.__hid_api.get_keymaps, None),
            _Subsystem.make(atx,          "ATX",       "atx_state"),
            _Subsystem.make(msd,          "MSD",       "msd_state"),
            _Subsystem.make(streamer,     "Streamer",  "streamer_state").add_source("streamer_ocr_state", self.__streamer_api.get_ocr, None),
            *[
                _Subsystem.make(info_manager.get_submanager(sub), f"Info manager ({sub})", f"info_{sub}_state",)
                for sub in sorted(info_manager.get_subs())
            ],
        ]

        self.__streamer_notifier = aiotools.AioNotifier()
        self.__reset_streamer = False
        self.__new_streamer_params: dict = {}

    # ===== STREAMER CONTROLLER

    @exposed_http("POST", "/streamer/set_params")
    async def __streamer_set_params_handler(self, request: Request) -> Response:
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
        self.__streamer_notifier.notify()
        return make_json_response()

    @exposed_http("POST", "/streamer/reset")
    async def __streamer_reset_handler(self, _: Request) -> Response:
        self.__reset_streamer = True
        self.__streamer_notifier.notify()
        return make_json_response()

    # ===== WEBSOCKET

    @exposed_http("GET", "/ws")
    async def __ws_handler(self, request: Request) -> WebSocketResponse:
        stream = valid_bool(request.query.get("stream", True))
        async with self._ws_session(request, stream=stream) as ws:
            states = [
                (event_type, src.get_state())
                for sub in self.__subsystems
                for (event_type, src) in sub.sources.items()
                if src.get_state
            ]
            events = dict(zip(
                map(operator.itemgetter(0), states),
                await asyncio.gather(*map(operator.itemgetter(1), states)),
            ))
            await asyncio.gather(*[
                ws.send_event(event_type, events.pop(event_type))
                for (event_type, _) in states
            ])
            await ws.send_event("loop", {})
            return (await self._ws_loop(ws))

    @exposed_ws("ping")
    async def __ws_ping_handler(self, ws: WsSession, _: dict) -> None:
        await ws.send_event("pong", {})

    # ===== SYSTEM STUFF

    def run(self, **kwargs: Any) -> None:  # type: ignore  # pylint: disable=arguments-differ
        for sub in self.__subsystems:
            if sub.sysprep:
                sub.sysprep()
        aioproc.rename_process("main")
        super().run(**kwargs)

    async def _check_request_auth(self, exposed: HttpExposed, request: Request) -> None:
        await check_request_auth(self.__auth_manager, exposed, request)

    async def _init_app(self) -> None:
        aiotools.create_deadly_task("Stream controller", self.__stream_controller())
        for sub in self.__subsystems:
            if sub.systask:
                aiotools.create_deadly_task(sub.name, sub.systask())
            for (event_type, src) in sub.sources.items():
                if src.poll_state:
                    aiotools.create_deadly_task(f"{sub.name} [poller]", self.__poll_state(event_type, src.poll_state()))
        aiotools.create_deadly_task("Stream snapshoter", self.__stream_snapshoter())
        self._add_exposed(*self.__apis)

    async def _on_shutdown(self) -> None:
        logger = get_logger(0)
        logger.info("Waiting short tasks ...")
        await aiotools.wait_all_short_tasks()
        logger.info("Stopping system tasks ...")
        await aiotools.stop_all_deadly_tasks()
        logger.info("Disconnecting clients ...")
        await self._close_all_wss()
        logger.info("On-Shutdown complete")

    async def _on_cleanup(self) -> None:
        logger = get_logger(0)
        for sub in self.__subsystems:
            if sub.cleanup:
                logger.info("Cleaning up %s ...", sub.name)
                try:
                    await sub.cleanup()  # type: ignore
                except Exception:
                    logger.exception("Cleanup error on %s", sub.name)
        logger.info("On-Cleanup complete")

    async def _on_ws_opened(self) -> None:
        self.__streamer_notifier.notify()

    async def _on_ws_closed(self) -> None:
        self.__hid.clear_events()
        self.__streamer_notifier.notify()

    def __has_stream_clients(self) -> bool:
        return bool(sum(map(
            (lambda ws: ws.kwargs["stream"]),
            self._get_wss(),
        )))

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

    async def __poll_state(self, event_type: str, poller: AsyncGenerator[dict, None]) -> None:
        async for state in poller:
            await self._broadcast_ws_event(event_type, state)

    async def __stream_snapshoter(self) -> None:
        await self.__snapshoter.run(
            is_live=self.__has_stream_clients,
            notifier=self.__streamer_notifier,
        )
