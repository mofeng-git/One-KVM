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


import os
import asyncio

from aiohttp.web import Request
from aiohttp.web import WebSocketResponse

from ...logging import get_logger

from ... import tools
from ... import aiotools
from ... import aiohelpers
from ... import fstab

from ...htserver import exposed_http
from ...htserver import exposed_ws
from ...htserver import WsSession
from ...htserver import HttpServer


# =====
class PstServer(HttpServer):  # pylint: disable=too-many-arguments,too-many-instance-attributes
    def __init__(  # pylint: disable=too-many-arguments,too-many-locals
        self,
        ro_retries_delay: float,
        ro_cleanup_delay: float,
        remount_cmd: list[str],
    ) -> None:

        super().__init__()

        self.__data_path = os.path.join(fstab.find_pst().root_path, "data")
        self.__ro_retries_delay = ro_retries_delay
        self.__ro_cleanup_delay = ro_cleanup_delay
        self.__remount_cmd = remount_cmd

        self.__notifier = aiotools.AioNotifier()

    # ===== WEBSOCKET

    @exposed_http("GET", "/ws")
    async def __ws_handler(self, request: Request) -> WebSocketResponse:
        async with self._ws_session(request) as ws:
            await ws.send_event("loop", {})
            return (await self._ws_loop(ws))

    @exposed_ws("ping")
    async def __ws_ping_handler(self, ws: WsSession, _: dict) -> None:
        await ws.send_event("pong", {})

    # ===== SYSTEM STUFF

    async def _init_app(self) -> None:
        if (await self.__remount_storage(rw=True)):
            await self.__remount_storage(rw=False)
        aiotools.create_deadly_task("Controller", self.__controller())
        self._add_exposed(self)

    async def _on_shutdown(self) -> None:
        logger = get_logger(0)
        logger.info("Stopping system tasks ...")
        await aiotools.stop_all_deadly_tasks()
        logger.info("Disconnecting clients ...")
        await self.__broadcast_storage_state(len(self._get_wss()), False)
        if (await self._close_all_wss()):
            await asyncio.sleep(self.__ro_cleanup_delay)
        logger.info("On-Shutdown complete")

    async def _on_cleanup(self) -> None:
        logger = get_logger(0)
        await self.__remount_storage(rw=False)
        logger.info("On-Cleanup complete")

    async def _on_ws_opened(self) -> None:
        self.__notifier.notify()

    async def _on_ws_closed(self) -> None:
        self.__notifier.notify()

    # ===== SYSTEM TASKS

    async def __controller(self) -> None:
        prev: int = 0
        while True:
            cur = len(self._get_wss())
            if cur > 0:
                if not self.__is_write_available():
                    await self.__remount_storage(rw=True)
            elif prev > 0 and cur == 0:
                while not (await self.__remount_storage(rw=False)):
                    if len(self._get_wss()) > 0:
                        continue
                    await asyncio.sleep(self.__ro_retries_delay)
            await self.__broadcast_storage_state(cur, self.__is_write_available())
            prev = cur
            await self.__notifier.wait()

    async def __broadcast_storage_state(self, clients: int, write_allowed: bool) -> None:
        await self._broadcast_ws_event("storage_state", {
            "clients": clients,
            "data": {
                "path": self.__data_path,
                "write_allowed": write_allowed,
            },
        })

    def __is_write_available(self) -> bool:
        try:
            return (not (os.statvfs(self.__data_path).f_flag & os.ST_RDONLY))
        except Exception as err:
            get_logger(0).info("Can't get filesystem state of PST (%s): %s",
                               self.__data_path, tools.efmt(err))
            return False

    async def __remount_storage(self, rw: bool) -> bool:
        return (await aiohelpers.remount("PST", self.__remount_cmd, rw))
