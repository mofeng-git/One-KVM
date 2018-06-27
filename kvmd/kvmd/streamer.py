import asyncio
import asyncio.subprocess

from typing import Dict
from typing import AsyncIterator
from typing import Optional

from contextlog import get_logger

from RPi import GPIO


# =====
class Streamer:
    def __init__(
        self,
        cap_power: int,
        vga_power: int,
        sync_delay: float,
        mjpg_streamer: Dict,
    ) -> None:

        self.__cap_power = self.__set_output_pin(cap_power)
        self.__vga_power = self.__set_output_pin(vga_power)
        self.__sync_delay = sync_delay

        self.__cmd = (
            "%(prog)s"
            " -i 'input_uvc.so -d %(device)s -e %(every)s -y -n -r %(width)sx%(height)s'"
            " -o 'output_http.so -p -l %(host)s %(port)s'"
        ) % (mjpg_streamer)

        self.__lock = asyncio.Lock()
        self.__events_queue: asyncio.Queue = asyncio.Queue()
        self.__proc_future: Optional[asyncio.Future] = None

    def __set_output_pin(self, pin: int) -> int:
        GPIO.setup(pin, GPIO.OUT)
        GPIO.output(pin, False)
        return pin

    async def events(self) -> AsyncIterator[str]:
        while True:
            yield (await self.__events_queue.get())

    async def start(self) -> None:
        async with self.__lock:
            get_logger().info("Starting mjpg_streamer ...")
            assert not self.__proc_future
            await self.__set_hw_enabled(True)
            self.__proc_future = asyncio.ensure_future(self.__process(), loop=asyncio.get_event_loop())

    async def stop(self) -> None:
        async with self.__lock:
            get_logger().info("Stopping mjpg_streamer ...")
            if self.__proc_future:
                self.__proc_future.cancel()
                await asyncio.gather(self.__proc_future, return_exceptions=True)
                await self.__set_hw_enabled(False)
                self.__proc_future = None
                await self.__events_queue.put("mjpg_streamer stopped")

    async def __set_hw_enabled(self, enabled: bool) -> None:
        # This sequence is important for enable
        GPIO.output(self.__cap_power, enabled)
        if enabled:
            await asyncio.sleep(self.__sync_delay)
        GPIO.output(self.__vga_power, enabled)
        await asyncio.sleep(self.__sync_delay)

    async def __process(self) -> None:
        logger = get_logger()

        proc: Optional[asyncio.subprocess.Process] = None  # pylint: disable=no-member
        while True:
            try:
                proc = await asyncio.create_subprocess_shell(
                    self.__cmd,
                    stdout=asyncio.subprocess.PIPE,
                    stderr=asyncio.subprocess.STDOUT,
                )

                logger.info("Started mjpg_streamer pid=%d: %s", proc.pid, self.__cmd)
                await self.__events_queue.put("mjpg_streamer started")

                empty = 0
                while proc.returncode is None:
                    line = (await proc.stdout.readline()).decode(errors="ignore").strip()
                    if line:
                        logger.info("mjpg_streamer: %s", line)
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
                    logger.error("Unexpected finished mjpg_streamer pid=%d with retcode=%d", proc.pid, proc.returncode)
                else:
                    logger.error("Can't start mjpg_streamer: %s", err)
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
