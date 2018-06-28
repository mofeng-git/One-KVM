import asyncio
import asyncio.subprocess

from typing import Optional

from .logging import get_logger

from . import gpio


# =====
class Streamer:  # pylint: disable=too-many-instance-attributes
    def __init__(
        self,
        cap_power: int,
        conv_power: int,
        sync_delay: float,
        cmd: str,
        loop: asyncio.AbstractEventLoop,
    ) -> None:

        self.__cap_power = gpio.set_output(cap_power)
        self.__conv_power = (gpio.set_output(conv_power) if conv_power > 0 else conv_power)
        self.__sync_delay = sync_delay
        self.__cmd = cmd

        self.__loop = loop

        self.__proc_task: Optional[asyncio.Task] = None

    async def start(self) -> None:
        assert not self.__proc_task
        get_logger().info("Starting streamer ...")
        await self.__set_hw_enabled(True)
        self.__proc_task = self.__loop.create_task(self.__process())

    async def stop(self) -> None:
        assert self.__proc_task
        get_logger().info("Stopping streamer ...")
        self.__proc_task.cancel()
        await asyncio.gather(self.__proc_task, return_exceptions=True)
        await self.__set_hw_enabled(False)
        self.__proc_task = None

    def is_running(self) -> bool:
        return bool(self.__proc_task)

    async def __set_hw_enabled(self, enabled: bool) -> None:
        # XXX: This sequence is very important to enable converter and cap board
        gpio.write(self.__cap_power, enabled)
        if self.__conv_power > 0:
            if enabled:
                await asyncio.sleep(self.__sync_delay)
            gpio.write(self.__conv_power, enabled)
        await asyncio.sleep(self.__sync_delay)

    async def __process(self) -> None:
        logger = get_logger(0)

        proc: Optional[asyncio.subprocess.Process] = None  # pylint: disable=no-member
        while True:
            try:
                proc = await asyncio.create_subprocess_shell(
                    self.__cmd,
                    stdout=asyncio.subprocess.PIPE,
                    stderr=asyncio.subprocess.STDOUT,
                )
                logger.info("Started streamer pid=%d: %s", proc.pid, self.__cmd)

                empty = 0
                while proc.returncode is None:
                    line = (await proc.stdout.readline()).decode(errors="ignore").strip()
                    if line:
                        logger.info("streamer: %s", line)
                        empty = 0
                    else:
                        empty += 1
                        if empty == 100:  # asyncio bug
                            break

                await self.__kill(proc)
                raise RuntimeError("WTF")

            except asyncio.CancelledError:
                break
            except Exception as err:
                if proc:
                    logger.exception("Unexpected finished streamer pid=%d with retcode=%d", proc.pid, proc.returncode)
                else:
                    logger.exception("Can't start streamer: %s", err)
                await asyncio.sleep(1)

        if proc:
            await self.__kill(proc)

    async def __kill(self, proc: asyncio.subprocess.Process) -> None:  # pylint: disable=no-member
        try:
            proc.terminate()
            await asyncio.sleep(1)
            if proc.returncode is None:
                proc.kill()
            await proc.wait()
        except Exception:
            pass
