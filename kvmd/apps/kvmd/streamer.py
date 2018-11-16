import os
import asyncio
import asyncio.subprocess

from typing import List
from typing import Dict
from typing import AsyncGenerator
from typing import Optional

import aiohttp

from ...logging import get_logger

from ... import gpio


# =====
class Streamer:  # pylint: disable=too-many-instance-attributes
    def __init__(  # pylint: disable=too-many-arguments,too-many-locals
        self,
        cap_power: int,
        conv_power: int,

        sync_delay: float,
        init_delay: float,
        init_restart_after: float,
        state_poll: float,

        quality: int,
        desired_fps: int,

        host: str,
        port: int,
        timeout: float,

        cmd: List[str],

        loop: asyncio.AbstractEventLoop,
        http_session: aiohttp.ClientSession,
    ) -> None:

        self.__cap_power = (gpio.set_output(cap_power) if cap_power > 0 else cap_power)
        self.__conv_power = (gpio.set_output(conv_power) if conv_power > 0 else conv_power)

        self.__sync_delay = sync_delay
        self.__init_delay = init_delay
        self.__init_restart_after = init_restart_after
        self.__state_poll = state_poll

        self.__params = {
            "quality": quality,
            "desired_fps": desired_fps,
        }

        self.__host = host
        self.__port = port
        self.__timeout = timeout

        self.__cmd = cmd

        self.__loop = loop
        self.__http_session = http_session

        self.__proc_task: Optional[asyncio.Task] = None

    async def start(self, params: Dict, no_init_restart: bool=False) -> None:
        logger = get_logger()
        logger.info("Starting streamer ...")

        self.__params = {key: params[key] for key in self.__params}  # Only known params
        assert 1 <= self.__params["quality"] <= 100
        assert 0 <= self.__params["desired_fps"] <= 30

        await self.__inner_start()
        if self.__init_restart_after > 0.0 and not no_init_restart:
            logger.info("Stopping streamer to restart ...")
            await self.__inner_stop()
            logger.info("Starting again ...")
            await self.__inner_start()

    async def stop(self) -> None:
        get_logger().info("Stopping streamer ...")
        await self.__inner_stop()

    def is_running(self) -> bool:
        return bool(self.__proc_task)

    def get_params(self) -> Dict:
        return dict(self.__params)

    async def get_state(self) -> Dict:
        url = "http://%s:%d/state" % (self.__host, self.__port)
        state = None
        try:
            async with self.__http_session.get(url, timeout=self.__timeout) as response:
                response.raise_for_status()
                state = (await response.json())["result"]
        except aiohttp.ClientConnectorError:
            pass
        except Exception:
            get_logger().exception("Invalid streamer response from /state")
        return {
            "params": self.get_params(),
            "state": state,
        }

    async def poll_state(self) -> AsyncGenerator[Dict, None]:
        while True:
            yield (await self.get_state())
            await asyncio.sleep(self.__state_poll)

    def get_app(self) -> str:
        return os.path.basename(self.__cmd[0])

    async def get_version(self) -> str:
        proc = await asyncio.create_subprocess_exec(
            *[self.__cmd[0], "--version"],
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.DEVNULL,
        )
        (stdout, _) = await proc.communicate()
        return stdout.decode(errors="ignore").strip()

    async def cleanup(self) -> None:
        if self.is_running():
            await self.stop()

    async def __inner_start(self) -> None:
        assert not self.__proc_task
        await self.__set_hw_enabled(True)
        self.__proc_task = self.__loop.create_task(self.__process())

    async def __inner_stop(self) -> None:
        assert self.__proc_task
        self.__proc_task.cancel()
        await asyncio.gather(self.__proc_task, return_exceptions=True)
        await self.__set_hw_enabled(False)
        self.__proc_task = None

    async def __set_hw_enabled(self, enabled: bool) -> None:
        # XXX: This sequence is very important to enable converter and cap board
        if self.__cap_power > 0:
            gpio.write(self.__cap_power, enabled)
        if self.__conv_power > 0:
            if enabled:
                await asyncio.sleep(self.__sync_delay)
            gpio.write(self.__conv_power, enabled)
        if enabled:
            await asyncio.sleep(self.__init_delay)

    async def __process(self) -> None:  # pylint: disable=too-many-branches
        logger = get_logger(0)

        while True:  # pylint: disable=too-many-nested-blocks
            proc: Optional[asyncio.subprocess.Process] = None  # pylint: disable=no-member
            try:
                cmd = [part.format(host=self.__host, port=self.__port, **self.__params) for part in self.__cmd]
                proc = await asyncio.create_subprocess_exec(
                    *cmd,
                    stdout=asyncio.subprocess.PIPE,
                    stderr=asyncio.subprocess.STDOUT,
                )
                logger.info("Started streamer pid=%d: %s", proc.pid, cmd)

                empty = 0
                async for line_bytes in proc.stdout:  # type: ignore
                    line = line_bytes.decode(errors="ignore").strip()
                    if line:
                        logger.info("Streamer: %s", line)
                        empty = 0
                    else:
                        empty += 1
                        if empty == 100:  # asyncio bug
                            raise RuntimeError("Streamer/asyncio: too many empty lines")

                raise RuntimeError("Streamer unexpectedly died")

            except asyncio.CancelledError:
                break

            except Exception as err:
                if proc:
                    logger.exception("Unexpected streamer error: pid=%d", proc.pid)
                else:
                    logger.exception("Can't start streamer: %s", err)
                await asyncio.sleep(1)

            finally:
                if proc and proc.returncode is None:
                    await self.__kill(proc)

    async def __kill(self, proc: asyncio.subprocess.Process) -> None:  # pylint: disable=no-member
        try:
            proc.terminate()
            await asyncio.sleep(1)
            if proc.returncode is None:
                try:
                    proc.kill()
                except Exception:
                    if proc.returncode is not None:
                        raise
            await proc.wait()
            get_logger().info("Streamer killed: pid=%d; retcode=%d", proc.pid, proc.returncode)
        except Exception:
            if proc.returncode is None:
                get_logger().exception("Can't kill streamer pid=%d", proc.pid)
            else:
                get_logger().info("Streamer killed: pid=%d; retcode=%d", proc.pid, proc.returncode)
