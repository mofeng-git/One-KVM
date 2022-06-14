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
import asyncio

from typing import List
from typing import Dict

from aiohttp.web import Request
from aiohttp.web import WebSocketResponse

from ...logging import get_logger

from ... import aiotools
from ... import aiohelpers

from ...htserver import exposed_http
from ...htserver import exposed_ws
from ...htserver import WsSession
from ...htserver import HttpServer


# =====
class PstServer(HttpServer):  # pylint: disable=too-many-arguments,too-many-instance-attributes
    def __init__(  # pylint: disable=too-many-arguments,too-many-locals
        self,
        storage_path: str,
        ro_retries_delay: float,
        remount_cmd: List[str],
    ) -> None:

        super().__init__()

        self.__data_path = os.path.join(storage_path, "data")
        self.__ro_retries_delay = ro_retries_delay
        self.__remount_cmd = remount_cmd

        self.__notifier = aiotools.AioNotifier()

    # ===== WEBSOCKET

    @exposed_http("GET", "/ws")
    async def __ws_handler(self, request: Request) -> WebSocketResponse:
        async with self._ws_session(request) as ws:
            await ws.send_event("loop", {})
            return (await self._ws_loop(ws))

    @exposed_ws("ping")
    async def __ws_ping_handler(self, ws: WsSession, _: Dict) -> None:
        await ws.send_event("pong", {})

    # ===== SYSTEM STUFF

    async def _init_app(self) -> None:
        if (await self.__remount_storage(True)):
            await self.__remount_storage(False)
        aiotools.create_deadly_task("Controller", self.__controller())
        self._add_exposed(self)

    async def _on_shutdown(self) -> None:
        logger = get_logger(0)
        logger.info("Stopping system tasks ...")
        await aiotools.stop_all_deadly_tasks()
        logger.info("Disconnecting clients ...")
        await self.__broadcast_storage_state(False)
        await self._close_all_wss()
        logger.info("On-Shutdown complete")

    async def _on_cleanup(self) -> None:
        logger = get_logger(0)
        await self.__remount_storage(False)
        logger.info("On-Cleanup complete")

    async def _on_ws_opened(self) -> None:
        await self.__notifier.notify()

    async def _on_ws_closed(self) -> None:
        await self.__notifier.notify()

    # ===== SYSTEM TASKS

    async def __controller(self) -> None:
        prev = False
        while True:
            cur = self.__has_clients()
            if not prev and cur:
                await self.__broadcast_storage_state(await self.__remount_storage(True))
            elif prev and not cur:
                while not (await self.__remount_storage(False)):
                    if self.__has_clients():
                        continue
                    await asyncio.sleep(self.__ro_retries_delay)
            prev = cur
            await self.__notifier.wait()

    def __has_clients(self) -> bool:
        return bool(self._get_wss())

    async def __broadcast_storage_state(self, write_allowed: bool) -> None:
        await self._broadcast_ws_event("storage_state", {
            "data": {"path": self.__data_path},
            "write_allowed": write_allowed,
        })

    async def __remount_storage(self, rw: bool) -> bool:
        return (await aiohelpers.remount("PST", self.__remount_cmd, rw))
