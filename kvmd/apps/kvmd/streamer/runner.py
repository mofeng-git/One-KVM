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


import asyncio
import asyncio.subprocess

from typing import Callable

from ....logging import get_logger

from .... import tools
from .... import aiotools
from .... import aioproc


# =====
class Runner:  # pylint: disable=too-many-instance-attributes
    def __init__(
        self,
        reset_delay: float,
        shutdown_delay: float,

        pre_start_cmd: list[str],
        cmd: list[str],
        post_stop_cmd: list[str],

        get_params: Callable[[], dict],
    ) -> None:

        self.__reset_delay = reset_delay
        self.__shutdown_delay = shutdown_delay

        self.__pre_start_cmd: list[str] = pre_start_cmd
        self.__cmd: list[str] = cmd
        self.__post_stop_cmd: list[str] = post_stop_cmd

        self.__get_params = get_params

        self.__proc_task: (asyncio.Task | None) = None
        self.__proc: (asyncio.subprocess.Process | None) = None  # pylint: disable=no-member

        self.__stopper_task: (asyncio.Task | None) = None
        self.__stopper_wip = False

    @aiotools.atomic_fg
    async def ensure_start(self, reset: bool) -> None:
        if not self.__proc_task or self.__stopper_task:
            logger = get_logger(0)

            if self.__stopper_task:
                if not self.__stopper_wip:
                    self.__stopper_task.cancel()
                    await asyncio.gather(self.__stopper_task, return_exceptions=True)
                    logger.info("Streamer stop cancelled")
                    return
                else:
                    await asyncio.gather(self.__stopper_task, return_exceptions=True)

            if reset and self.__reset_delay > 0:
                logger.info("Waiting %.2f seconds for reset delay ...", self.__reset_delay)
                await asyncio.sleep(self.__reset_delay)
            logger.info("Starting streamer ...")
            await self.__inner_start()

    @aiotools.atomic_fg
    async def ensure_stop(self, immediately: bool) -> None:
        if self.__proc_task:
            logger = get_logger(0)

            if immediately:
                if self.__stopper_task:
                    if not self.__stopper_wip:
                        self.__stopper_task.cancel()
                        await asyncio.gather(self.__stopper_task, return_exceptions=True)
                        logger.info("Stopping streamer immediately ...")
                        await self.__inner_stop()
                    else:
                        await asyncio.gather(self.__stopper_task, return_exceptions=True)
                else:
                    logger.info("Stopping streamer immediately ...")
                    await self.__inner_stop()

            elif not self.__stopper_task:

                async def delayed_stop() -> None:
                    try:
                        await asyncio.sleep(self.__shutdown_delay)
                        self.__stopper_wip = True
                        logger.info("Stopping streamer after delay ...")
                        await self.__inner_stop()
                    finally:
                        self.__stopper_task = None
                        self.__stopper_wip = False

                logger.info("Planning to stop streamer in %.2f seconds ...", self.__shutdown_delay)
                self.__stopper_task = asyncio.create_task(delayed_stop())

    def is_working(self) -> bool:
        # Запущено и не планирует останавливаться
        return bool(self.__proc_task and not self.__stopper_task)

    # =====

    def _is_alive(self) -> bool:
        return bool(self.__proc_task)

    @aiotools.atomic_fg
    async def __inner_start(self) -> None:
        assert not self.__proc_task
        await self.__run_hook("PRE-START-CMD", self.__pre_start_cmd)
        self.__proc_task = asyncio.create_task(self.__process_task_loop())

    @aiotools.atomic_fg
    async def __inner_stop(self) -> None:
        assert self.__proc_task
        self.__proc_task.cancel()
        await asyncio.gather(self.__proc_task, return_exceptions=True)
        await self.__kill_process()
        await self.__run_hook("POST-STOP-CMD", self.__post_stop_cmd)
        self.__proc_task = None

    # =====

    async def __process_task_loop(self) -> None:  # pylint: disable=too-many-branches
        logger = get_logger(0)
        while True:  # pylint: disable=too-many-nested-blocks
            try:
                await self.__start_process()
                assert self.__proc is not None
                await aioproc.log_stdout_infinite(self.__proc, logger)
                raise RuntimeError("Streamer unexpectedly died")
            except asyncio.CancelledError:
                break
            except Exception:
                if self.__proc:
                    logger.exception("Unexpected streamer error: pid=%d", self.__proc.pid)
                else:
                    logger.exception("Can't start streamer")
                await self.__kill_process()
                await asyncio.sleep(1)

    def __make_cmd(self, cmd: list[str]) -> list[str]:
        params = self.__get_params()
        return [part.format(**params) for part in cmd]

    async def __run_hook(self, name: str, cmd: list[str]) -> None:
        logger = get_logger()
        cmd = self.__make_cmd(cmd)
        logger.info("%s: %s", name, tools.cmdfmt(cmd))
        try:
            await aioproc.log_process(cmd, logger, prefix=name)
        except Exception:
            logger.exception("Can't execute %s hook: %s", name, tools.cmdfmt(cmd))

    async def __start_process(self) -> None:
        assert self.__proc is None
        cmd = self.__make_cmd(self.__cmd)
        self.__proc = await aioproc.run_process(cmd)
        get_logger(0).info("Started streamer pid=%d: %s", self.__proc.pid, tools.cmdfmt(cmd))

    async def __kill_process(self) -> None:
        if self.__proc:
            await aioproc.kill_process(self.__proc, 1, get_logger(0))
        self.__proc = None
