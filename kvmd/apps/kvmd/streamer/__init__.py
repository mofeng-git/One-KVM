# ========================================================================== #
#                                                                            #
#    KVMD - The main PiKVM daemon.                                           #
#                                                                            #
#    Copyright (C) 2018-2024  Maxim Devaev <mdevaev@gmail.com>               #
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


import signal
import asyncio
import dataclasses
import copy

from typing import AsyncGenerator
from typing import Any

import aiohttp

from ....logging import get_logger

from ....clients.streamer import StreamerSnapshot
from ....clients.streamer import HttpStreamerClient
from ....clients.streamer import HttpStreamerClientSession

from .... import tools
from .... import aiotools
from .... import htclient

from .params import Params
from .runner import Runner


# =====
class Streamer:  # pylint: disable=too-many-instance-attributes
    __ST_FULL     = 0xFF
    __ST_PARAMS   = 0x01
    __ST_STREAMER = 0x02
    __ST_SNAPSHOT = 0x04

    def __init__(  # pylint: disable=too-many-arguments,too-many-locals
        self,

        reset_delay: float,
        shutdown_delay: float,
        state_poll: float,

        unix_path: str,
        timeout: float,
        snapshot_timeout: float,

        process_name_prefix: str,

        pre_start_cmd: list[str],
        pre_start_cmd_remove: list[str],
        pre_start_cmd_append: list[str],

        cmd: list[str],
        cmd_remove: list[str],
        cmd_append: list[str],

        post_stop_cmd: list[str],
        post_stop_cmd_remove: list[str],
        post_stop_cmd_append: list[str],

        **params_kwargs: Any,
    ) -> None:

        self.__state_poll = state_poll

        self.__unix_path = unix_path
        self.__snapshot_timeout = snapshot_timeout
        self.__process_name_prefix = process_name_prefix

        self.__params = Params(**params_kwargs)

        self.__runner = Runner(
            reset_delay=reset_delay,
            shutdown_delay=shutdown_delay,
            pre_start_cmd=tools.build_cmd(pre_start_cmd, pre_start_cmd_remove, pre_start_cmd_append),
            cmd=tools.build_cmd(cmd, cmd_remove, cmd_append),
            post_stop_cmd=tools.build_cmd(post_stop_cmd, post_stop_cmd_remove, post_stop_cmd_append),
        )

        self.__client = HttpStreamerClient(
            name="jpeg",
            unix_path=self.__unix_path,
            timeout=timeout,
            user_agent=htclient.make_user_agent("KVMD"),
        )
        self.__client_session: (HttpStreamerClientSession | None) = None

        self.__snapshot: (StreamerSnapshot | None) = None

        self.__notifier = aiotools.AioNotifier()

    # =====

    @aiotools.atomic_fg
    async def ensure_start(self) -> None:
        await self.__runner.ensure_start(self.__make_params())

    @aiotools.atomic_fg
    async def ensure_restart(self) -> None:
        await self.__runner.ensure_restart(self.__make_params())

    def __make_params(self) -> dict:
        return {
            "unix": self.__unix_path,
            "process_name_prefix": self.__process_name_prefix,
            **self.__params.get_params(),
        }

    @aiotools.atomic_fg
    async def ensure_stop(self) -> None:
        await self.__runner.ensure_stop(immediately=False)

    # =====

    def set_params(self, params: dict) -> None:
        self.__notifier.notify(self.__ST_PARAMS)
        return self.__params.set_params(params)

    def get_params(self) -> dict:
        return self.__params.get_params()

    # =====

    async def get_state(self) -> dict:
        return {
            "features": self.__params.get_features(),
            "limits": self.__params.get_limits(),
            "params": self.__params.get_params(),
            "streamer": (await self.__get_streamer_state()),
            "snapshot": self.__get_snapshot_state(),
        }

    async def trigger_state(self) -> None:
        self.__notifier.notify(self.__ST_FULL)

    async def poll_state(self) -> AsyncGenerator[dict, None]:
        # ==== Granularity table ====
        #   - features -- Full
        #   - limits   -- Partial, paired with params
        #   - params   -- Partial, paired with limits
        #   - streamer -- Partial, nullable
        #   - snapshot -- Partial
        # ===========================

        def signal_handler(*_: Any) -> None:
            get_logger(0).info("Got SIGUSR2, checking the stream state ...")
            self.__notifier.notify(self.__ST_STREAMER)

        get_logger(0).info("Installing SIGUSR2 streamer handler ...")
        asyncio.get_event_loop().add_signal_handler(signal.SIGUSR2, signal_handler)

        prev: dict = {}
        while True:
            new: dict = {}

            mask = await self.__notifier.wait(timeout=self.__state_poll)
            if mask == self.__ST_FULL:
                new = await self.get_state()
                prev = copy.deepcopy(new)
                yield new
                continue

            if mask < 0:
                mask = self.__ST_STREAMER

            def check_update(key: str, value: (dict | None)) -> None:
                if prev.get(key) != value:
                    new[key] = value

            if mask & self.__ST_PARAMS:
                check_update("params", self.__params.get_params())
            if mask & self.__ST_STREAMER:
                check_update("streamer", await self.__get_streamer_state())
            if mask & self.__ST_SNAPSHOT:
                check_update("snapshot", self.__get_snapshot_state())

            if new and prev != new:
                prev.update(copy.deepcopy(new))
                yield new

    async def __get_streamer_state(self) -> (dict | None):
        if self.__runner.is_running():
            session = self.__ensure_client_session()
            try:
                return (await session.get_state())
            except (aiohttp.ClientConnectionError, aiohttp.ServerConnectionError):
                pass
            except Exception:
                get_logger().exception("Invalid streamer response from /state")
        return None

    def __get_snapshot_state(self) -> dict:
        if self.__snapshot:
            snapshot = dataclasses.asdict(self.__snapshot)
            del snapshot["headers"]
            del snapshot["data"]
            return {"saved": snapshot}
        return {"saved": None}

    # =====

    async def take_snapshot(self, save: bool, load: bool, allow_offline: bool) -> (StreamerSnapshot | None):
        if load:
            return self.__snapshot
        logger = get_logger()
        session = self.__ensure_client_session()
        try:
            snapshot = await session.take_snapshot(self.__snapshot_timeout)
            if snapshot.online or allow_offline:
                if save:
                    self.__snapshot = snapshot
                    self.__notifier.notify(self.__ST_SNAPSHOT)
                return snapshot
            logger.error("Stream is offline, no signal or so")
        except (aiohttp.ClientConnectionError, aiohttp.ServerConnectionError) as ex:
            logger.error("Can't connect to streamer: %s", tools.efmt(ex))
        except Exception:
            logger.exception("Invalid streamer response from /snapshot")
        return None

    def remove_snapshot(self) -> None:
        self.__snapshot = None

    # =====

    @aiotools.atomic_fg
    async def cleanup(self) -> None:
        await self.__runner.ensure_stop(immediately=True)
        if self.__client_session:
            await self.__client_session.close()
            self.__client_session = None

    def __ensure_client_session(self) -> HttpStreamerClientSession:
        if not self.__client_session:
            self.__client_session = self.__client.make_session()
        return self.__client_session
