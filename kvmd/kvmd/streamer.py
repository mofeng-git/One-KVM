import asyncio
import asyncio.subprocess
import logging

from typing import Dict
from typing import Optional

from RPi import GPIO


# =====
_logger = logging.getLogger(__name__)


class Streamer:  # pylint: disable=too-many-instance-attributes
    def __init__(
        self,
        cap_power: int,
        vga_power: int,
        sync_delay: float,
        mjpg_streamer: Dict,
        loop: asyncio.AbstractEventLoop,
    ) -> None:

        self.__cap_power = self.__set_output_pin(cap_power)
        self.__vga_power = self.__set_output_pin(vga_power)
        self.__sync_delay = sync_delay

        self.__cmd = (
            "%(prog)s"
            " -i 'input_uvc.so -d %(device)s -e %(every)s -y -n -r %(width)sx%(height)s'"
            " -o 'output_http.so -p -l %(host)s %(port)s'"
        ) % (mjpg_streamer)

        self.__loop = loop

        self.__proc_task: Optional[asyncio.Task] = None

    def __set_output_pin(self, pin: int) -> int:
        GPIO.setup(pin, GPIO.OUT)
        GPIO.output(pin, False)
        return pin

    async def start(self) -> None:
        assert not self.__proc_task
        _logger.info("Starting mjpg_streamer ...")
        await self.__set_hw_enabled(True)
        self.__proc_task = self.__loop.create_task(self.__process())

    async def stop(self) -> None:
        assert self.__proc_task
        _logger.info("Stopping mjpg_streamer ...")
        self.__proc_task.cancel()
        await asyncio.gather(self.__proc_task, return_exceptions=True)
        await self.__set_hw_enabled(False)
        self.__proc_task = None

    def is_running(self) -> bool:
        return bool(self.__proc_task)

    async def __set_hw_enabled(self, enabled: bool) -> None:
        # XXX: This sequence is very important for enable
        GPIO.output(self.__cap_power, enabled)
        if enabled:
            await asyncio.sleep(self.__sync_delay)
        GPIO.output(self.__vga_power, enabled)
        await asyncio.sleep(self.__sync_delay)

    async def __process(self) -> None:
        proc: Optional[asyncio.subprocess.Process] = None  # pylint: disable=no-member
        while True:
            try:
                proc = await asyncio.create_subprocess_shell(
                    self.__cmd,
                    stdout=asyncio.subprocess.PIPE,
                    stderr=asyncio.subprocess.STDOUT,
                )
                _logger.info("Started mjpg_streamer pid=%d: %s", proc.pid, self.__cmd)

                empty = 0
                while proc.returncode is None:
                    line = (await proc.stdout.readline()).decode(errors="ignore").strip()
                    if line:
                        _logger.info("mjpg_streamer: %s", line)
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
                    _logger.error("Unexpected finished mjpg_streamer pid=%d with retcode=%d", proc.pid, proc.returncode)
                else:
                    _logger.error("Can't start mjpg_streamer: %s", err)
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
