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
import asyncio.subprocess

from typing import List
from typing import Dict
from typing import AsyncGenerator
from typing import Optional
from typing import Any

import aiohttp

from ...logging import get_logger

from ... import aiotools
from ... import gpio

from ... import __version__


# =====
class Streamer:  # pylint: disable=too-many-instance-attributes
    def __init__(  # pylint: disable=too-many-arguments,too-many-locals
        self,
        cap_pin: int,
        conv_pin: int,

        sync_delay: float,
        init_delay: float,
        init_restart_after: float,
        shutdown_delay: float,
        state_poll: float,

        quality: int,
        desired_fps: int,
        max_fps: int,

        host: str,
        port: int,
        unix_path: str,
        timeout: float,

        process_name_prefix: str,

        cmd: List[str],
    ) -> None:

        self.__cap_pin = (gpio.set_output(cap_pin) if cap_pin >= 0 else -1)
        self.__conv_pin = (gpio.set_output(conv_pin) if conv_pin >= 0 else -1)

        self.__sync_delay = sync_delay
        self.__init_delay = init_delay
        self.__init_restart_after = init_restart_after
        self.__shutdown_delay = shutdown_delay
        self.__state_poll = state_poll

        self.__params = {
            "quality": quality,
            "desired_fps": desired_fps,
        }
        self.__max_fps = max_fps

        assert port or unix_path
        self.__host = host
        self.__port = port
        self.__unix_path = unix_path
        self.__timeout = timeout

        self.__process_name_prefix = process_name_prefix

        self.__cmd = cmd

        self.__stop_task: Optional[asyncio.Task] = None
        self.__stop_wip = False

        self.__streamer_task: Optional[asyncio.Task] = None
        self.__streamer_proc: Optional[asyncio.subprocess.Process] = None  # pylint: disable=no-member

        self.__http_session: Optional[aiohttp.ClientSession] = None

    # =====

    @aiotools.atomic
    async def ensure_start(self, init_restart: bool) -> None:
        if not self.__streamer_task or self.__stop_task:
            logger = get_logger(0)

            if self.__stop_task:
                if not self.__stop_wip:
                    self.__stop_task.cancel()
                    await asyncio.gather(self.__stop_task, return_exceptions=True)
                    logger.info("Streamer stop cancelled")
                    return
                else:
                    await asyncio.gather(self.__stop_task, return_exceptions=True)

            logger.info("Starting streamer ...")
            await self.__inner_start()
            if self.__init_restart_after > 0.0 and init_restart:
                await asyncio.sleep(self.__init_restart_after)
                logger.info("Stopping streamer to restart ...")
                await self.__inner_stop()
                logger.info("Starting again ...")
                await self.__inner_start()

    @aiotools.atomic
    async def ensure_stop(self, immediately: bool) -> None:
        if self.__streamer_task:
            logger = get_logger(0)

            if immediately:
                if self.__stop_task:
                    if not self.__stop_wip:
                        self.__stop_task.cancel()
                        await asyncio.gather(self.__stop_task, return_exceptions=True)
                        logger.info("Stopping streamer immediately ...")
                        await self.__inner_stop()
                    else:
                        await asyncio.gather(self.__stop_task, return_exceptions=True)
                else:
                    logger.info("Stopping streamer immediately ...")
                    await self.__inner_stop()

            elif not self.__stop_task:

                async def delayed_stop() -> None:
                    try:
                        await asyncio.sleep(self.__shutdown_delay)
                        self.__stop_wip = True
                        logger.info("Stopping streamer after delay ...")
                        await self.__inner_stop()
                    finally:
                        self.__stop_task = None
                        self.__stop_wip = False

                logger.info("Planning to stop streamer in %.2f seconds ...", self.__shutdown_delay)
                self.__stop_task = asyncio.create_task(delayed_stop())

    def is_working(self) -> bool:
        # Запущено и не планирует останавливаться
        return bool(self.__streamer_task and not self.__stop_task)

    def set_params(self, params: Dict) -> None:
        assert not self.__streamer_task
        self.__params = {
            key: min(max(params.get(key, self.__params[key]), a), b)
            for (key, a, b) in [
                ("quality", 0, 100),
                ("desired_fps", 0, self.__max_fps),
            ]
        }

    async def get_state(self) -> Dict:
        state = None
        if self.__streamer_task:
            session = self.__ensure_session()
            try:
                async with session.get(
                    url=f"http://{self.__host}:{self.__port}/state",
                    headers={"User-Agent": f"KVMD/{__version__}"},
                    timeout=self.__timeout,
                ) as response:
                    response.raise_for_status()
                    state = (await response.json())["result"]
            except (aiohttp.ClientConnectionError, aiohttp.ServerConnectionError):
                pass
            except asyncio.CancelledError:  # pylint: disable=try-except-raise
                raise
            except Exception:
                get_logger().exception("Invalid streamer response from /state")
        return {
            "limits": {"max_fps": self.__max_fps},
            "params": self.__params,
            "state": state,
        }

    async def poll_state(self) -> AsyncGenerator[Dict, None]:
        notifier = aiotools.AioNotifier()

        def signal_handler(*_: Any) -> None:
            get_logger(0).info("Got SIGUSR2, checking the stream state ...")
            asyncio.ensure_future(notifier.notify())

        get_logger(0).info("Installing SIGUSR2 streamer handler ...")
        asyncio.get_event_loop().add_signal_handler(signal.SIGUSR2, signal_handler)

        waiter_task: Optional[asyncio.Task] = None
        prev_state: Dict = {}
        while True:
            state = await self.get_state()
            if state != prev_state:
                yield state
                prev_state = state

            if waiter_task is None:
                waiter_task = asyncio.create_task(notifier.wait())
            if waiter_task in (await aiotools.wait_first(asyncio.sleep(self.__state_poll), waiter_task))[0]:
                waiter_task = None

    async def get_info(self) -> Dict:
        proc = await asyncio.create_subprocess_exec(
            *[self.__cmd[0], "--version"],
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.DEVNULL,
            preexec_fn=(lambda: signal.signal(signal.SIGINT, signal.SIG_IGN)),
        )
        (stdout, _) = await proc.communicate()
        return {
            "app": os.path.basename(self.__cmd[0]),
            "version": stdout.decode(errors="ignore").strip(),
        }

    @aiotools.atomic
    async def cleanup(self) -> None:
        try:
            await self.ensure_stop(immediately=True)
            if self.__http_session:
                await self.__http_session.close()
                self.__http_session = None
        finally:
            await self.__set_hw_enabled(False)

    # =====

    def __ensure_session(self) -> aiohttp.ClientSession:
        if not self.__http_session:
            if self.__unix_path:
                self.__http_session = aiohttp.ClientSession(connector=aiohttp.UnixConnector(path=self.__unix_path))
            else:
                self.__http_session = aiohttp.ClientSession()
        return self.__http_session

    # =====

    @aiotools.atomic
    async def __inner_start(self) -> None:
        assert not self.__streamer_task
        await self.__set_hw_enabled(True)
        self.__streamer_task = asyncio.create_task(self.__streamer_task_loop())

    @aiotools.atomic
    async def __inner_stop(self) -> None:
        assert self.__streamer_task
        self.__streamer_task.cancel()
        await asyncio.gather(self.__streamer_task, return_exceptions=True)
        await self.__kill_streamer_proc()
        await self.__set_hw_enabled(False)
        self.__streamer_task = None

    @aiotools.atomic
    async def __set_hw_enabled(self, enabled: bool) -> None:
        # XXX: This sequence is very important to enable converter and cap board
        if self.__cap_pin >= 0:
            gpio.write(self.__cap_pin, enabled)
        if self.__conv_pin >= 0:
            if enabled:
                await asyncio.sleep(self.__sync_delay)
            gpio.write(self.__conv_pin, enabled)
        if enabled:
            await asyncio.sleep(self.__init_delay)

    # =====

    async def __streamer_task_loop(self) -> None:  # pylint: disable=too-many-branches
        logger = get_logger(0)
        while True:  # pylint: disable=too-many-nested-blocks
            try:
                await self.__start_streamer_proc()

                empty = 0
                async for line_bytes in self.__streamer_proc.stdout:  # type: ignore
                    line = line_bytes.decode(errors="ignore").strip()
                    if line:
                        logger.info("Console: %s", line)
                        empty = 0
                    else:
                        empty += 1
                        if empty == 100:  # asyncio bug
                            raise RuntimeError("Streamer/asyncio: too many empty lines")

                raise RuntimeError("Streamer unexpectedly died")

            except asyncio.CancelledError:
                break

            except Exception as err:
                if self.__streamer_proc:
                    logger.exception("Unexpected streamer error: pid=%d", self.__streamer_proc.pid)
                else:
                    logger.exception("Can't start streamer: %s", err)
                await self.__kill_streamer_proc()
                await asyncio.sleep(1)

    async def __start_streamer_proc(self) -> None:
        assert self.__streamer_proc is None
        cmd = [
            part.format(
                host=self.__host,
                port=self.__port,
                unix=self.__unix_path,
                process_name_prefix=self.__process_name_prefix,
                **self.__params,
            )
            for part in self.__cmd
        ]
        self.__streamer_proc = await asyncio.create_subprocess_exec(
            *cmd,
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.STDOUT,
            preexec_fn=(lambda: signal.signal(signal.SIGINT, signal.SIG_IGN)),
        )
        get_logger(0).info("Started streamer pid=%d: %s", self.__streamer_proc.pid, cmd)

    async def __kill_streamer_proc(self) -> None:
        logger = get_logger(0)
        if self.__streamer_proc and self.__streamer_proc.returncode is None:
            try:
                self.__streamer_proc.terminate()
                await asyncio.sleep(1)
                if self.__streamer_proc.returncode is None:
                    try:
                        self.__streamer_proc.kill()
                    except Exception:
                        if self.__streamer_proc.returncode is not None:
                            raise
                await self.__streamer_proc.wait()
                logger.info("Streamer killed: pid=%d; retcode=%d",
                            self.__streamer_proc.pid, self.__streamer_proc.returncode)
            except asyncio.CancelledError:
                pass
            except Exception:
                if self.__streamer_proc.returncode is None:
                    logger.exception("Can't kill streamer pid=%d", self.__streamer_proc.pid)
                else:
                    logger.info("Streamer killed: pid=%d; retcode=%d",
                                self.__streamer_proc.pid, self.__streamer_proc.returncode)
        self.__streamer_proc = None
