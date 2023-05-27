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
import time

from typing import Callable

from ...logging import get_logger

from ... import aiotools

from ...plugins.hid import BaseHid

from .streamer import Streamer


# =====
class Snapshoter:  # pylint: disable=too-many-instance-attributes
    def __init__(
        self,
        hid: BaseHid,
        streamer: Streamer,

        idle_interval: float,
        live_interval: float,

        wakeup_key: str,
        wakeup_move: int,

        online_delay: float,
        retries: int,
        retries_delay: float,
    ) -> None:

        self.__hid = hid
        self.__streamer = streamer

        if idle_interval or live_interval:
            self.__idle_interval = (idle_interval or live_interval)
            self.__live_interval = (live_interval or idle_interval)
            assert self.__idle_interval
            assert self.__live_interval
        else:
            self.__idle_interval = self.__live_interval = 0.0

        self.__wakeup_key = wakeup_key
        self.__wakeup_move = wakeup_move

        self.__online_delay = online_delay
        self.__retries = retries
        self.__retries_delay = retries_delay

        self.__snapshoting = False

    async def run(self, is_live: Callable[[], bool], notifier: aiotools.AioNotifier) -> None:
        if self.__idle_interval or self.__live_interval:
            get_logger(0).info("Running periodic stream snapshot: idle=%.2f; live=%.2f ...",
                               self.__idle_interval, self.__live_interval)

            last_snapshot_ts = 0.0
            while True:
                live = is_live()
                if last_snapshot_ts + (self.__live_interval if live else self.__idle_interval) < time.monotonic():
                    await self.__take_snapshot(live, notifier)
                    last_snapshot_ts = time.monotonic()
                await asyncio.sleep(min(self.__idle_interval, self.__live_interval))
        else:
            await aiotools.wait_infinite()

    def snapshoting(self) -> bool:
        return self.__snapshoting

    async def __take_snapshot(self, live: bool, notifier: aiotools.AioNotifier) -> None:
        logger = get_logger(0)
        if not live:
            logger.info("Time to take the new idle snapshot")
        try:
            self.__snapshoting = True
            notifier.notify()

            if not live:
                await self.__wakeup()

            retries = self.__retries
            while retries:
                snapshot = await self.__streamer.take_snapshot(save=True, load=False, allow_offline=False)
                if snapshot:
                    if not live:
                        logger.info("New idle snapshot saved: %dx%d", snapshot.width, snapshot.height)
                    break
                retries -= 1
                await asyncio.sleep(self.__retries_delay)
            else:
                logger.error("Can't take snapshot after %d retries", self.__retries)
        except Exception:  # Этого вообще-то не должно случаться, апи внутри заэксцепчены, но мало ли
            logger.exception("Unhandled exception while taking snapshot")
        finally:
            self.__snapshoting = False
            notifier.notify()

    async def __wakeup(self) -> None:
        logger = get_logger(0)

        if self.__wakeup_key:
            logger.info("Waking up using key %r ...", self.__wakeup_key)
            self.__hid.send_key_events([
                (self.__wakeup_key, True),
                (self.__wakeup_key, False),
            ])

        if self.__wakeup_move:
            logger.info("Waking up using mouse move for %d units ...", self.__wakeup_move)
            for (to_x, to_y) in [(0, 0), (self.__wakeup_move, self.__wakeup_move), (0, 0)]:
                self.__hid.send_mouse_move_event(to_x, to_y)

        if self.__online_delay:
            logger.info("Waiting %.2f seconds for online ...", self.__online_delay)
            await asyncio.sleep(self.__online_delay)
