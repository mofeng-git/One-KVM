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


import io
import signal
import asyncio
import asyncio.subprocess
import dataclasses
import functools

from typing import AsyncGenerator
from typing import Any

import aiohttp

from PIL import Image as PilImage

from ...logging import get_logger

from ... import tools
from ... import aiotools
from ... import aioproc
from ... import htclient


# =====
@dataclasses.dataclass(frozen=True)
class StreamerSnapshot:
    online: bool
    width: int
    height: int
    headers: tuple[tuple[str, str], ...]
    data: bytes

    async def make_preview(self, max_width: int, max_height: int, quality: int) -> bytes:
        assert max_width >= 0
        assert max_height >= 0
        assert quality > 0

        if max_width == 0 and max_height == 0:
            max_width = self.width // 5
            max_height = self.height // 5
        else:
            max_width = min((max_width or self.width), self.width)
            max_height = min((max_height or self.height), self.height)

        if (max_width, max_height) == (self.width, self.height):
            return self.data
        return (await aiotools.run_async(self.__inner_make_preview, max_width, max_height, quality))

    @functools.lru_cache(maxsize=1)
    def __inner_make_preview(self, max_width: int, max_height: int, quality: int) -> bytes:
        with io.BytesIO(self.data) as snapshot_bio:
            with io.BytesIO() as preview_bio:
                with PilImage.open(snapshot_bio) as image:
                    image.thumbnail((max_width, max_height), PilImage.Resampling.LANCZOS)
                    image.save(preview_bio, format="jpeg", quality=quality)
                    return preview_bio.getvalue()


class _StreamerParams:
    __DESIRED_FPS = "desired_fps"

    __QUALITY = "quality"

    __RESOLUTION = "resolution"
    __AVAILABLE_RESOLUTIONS = "available_resolutions"

    __H264_BITRATE = "h264_bitrate"
    __H264_GOP = "h264_gop"

    def __init__(  # pylint: disable=too-many-arguments
        self,
        quality: int,

        resolution: str,
        available_resolutions: list[str],

        desired_fps: int,
        desired_fps_min: int,
        desired_fps_max: int,

        h264_bitrate: int,
        h264_bitrate_min: int,
        h264_bitrate_max: int,

        h264_gop: int,
        h264_gop_min: int,
        h264_gop_max: int,
    ) -> None:

        self.__has_quality = bool(quality)
        self.__has_resolution = bool(resolution)
        self.__has_h264 = bool(h264_bitrate)

        self.__params: dict = {self.__DESIRED_FPS: min(max(desired_fps, desired_fps_min), desired_fps_max)}
        self.__limits: dict = {self.__DESIRED_FPS: {"min": desired_fps_min, "max": desired_fps_max}}

        if self.__has_quality:
            self.__params[self.__QUALITY] = quality

        if self.__has_resolution:
            self.__params[self.__RESOLUTION] = resolution
            self.__limits[self.__AVAILABLE_RESOLUTIONS] = available_resolutions

        if self.__has_h264:
            self.__params[self.__H264_BITRATE] = min(max(h264_bitrate, h264_bitrate_min), h264_bitrate_max)
            self.__limits[self.__H264_BITRATE] = {"min": h264_bitrate_min, "max": h264_bitrate_max}
            self.__params[self.__H264_GOP] = min(max(h264_gop, h264_gop_min), h264_gop_max)
            self.__limits[self.__H264_GOP] = {"min": h264_gop_min, "max": h264_gop_max}

    def get_features(self) -> dict:
        return {
            self.__QUALITY: self.__has_quality,
            self.__RESOLUTION: self.__has_resolution,
            "h264": self.__has_h264,
        }

    def get_limits(self) -> dict:
        limits = dict(self.__limits)
        if self.__has_resolution:
            limits[self.__AVAILABLE_RESOLUTIONS] = list(limits[self.__AVAILABLE_RESOLUTIONS])
        return limits

    def get_params(self) -> dict:
        return dict(self.__params)

    def set_params(self, params: dict) -> None:
        new_params = dict(self.__params)

        if self.__QUALITY in params and self.__has_quality:
            new_params[self.__QUALITY] = min(max(params[self.__QUALITY], 1), 100)

        if self.__RESOLUTION in params and self.__has_resolution:
            if params[self.__RESOLUTION] in self.__limits[self.__AVAILABLE_RESOLUTIONS]:
                new_params[self.__RESOLUTION] = params[self.__RESOLUTION]

        for (key, enabled) in [
            (self.__DESIRED_FPS, True),
            (self.__H264_BITRATE, self.__has_h264),
            (self.__H264_GOP, self.__has_h264),
        ]:
            if key in params and enabled:
                if self.__check_limits_min_max(key, params[key]):
                    new_params[key] = params[key]

        self.__params = new_params

    def __check_limits_min_max(self, key: str, value: int) -> bool:
        return (self.__limits[key]["min"] <= value <= self.__limits[key]["max"])


class Streamer:  # pylint: disable=too-many-instance-attributes
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

        self.__reset_delay = reset_delay
        self.__shutdown_delay = shutdown_delay
        self.__state_poll = state_poll

        self.__unix_path = unix_path
        self.__timeout = timeout
        self.__snapshot_timeout = snapshot_timeout

        self.__process_name_prefix = process_name_prefix

        self.__pre_start_cmd = tools.build_cmd(pre_start_cmd, pre_start_cmd_remove, pre_start_cmd_append)
        self.__cmd = tools.build_cmd(cmd, cmd_remove, cmd_append)
        self.__post_stop_cmd = tools.build_cmd(post_stop_cmd, post_stop_cmd_remove, post_stop_cmd_append)

        self.__params = _StreamerParams(**params_kwargs)

        self.__stop_task: (asyncio.Task | None) = None
        self.__stop_wip = False

        self.__streamer_task: (asyncio.Task | None) = None
        self.__streamer_proc: (asyncio.subprocess.Process | None) = None  # pylint: disable=no-member

        self.__http_session: (aiohttp.ClientSession | None) = None

        self.__snapshot: (StreamerSnapshot | None) = None

        self.__notifier = aiotools.AioNotifier()

    # =====

    @aiotools.atomic_fg
    async def ensure_start(self, reset: bool) -> None:
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

            if reset and self.__reset_delay > 0:
                logger.info("Waiting %.2f seconds for reset delay ...", self.__reset_delay)
                await asyncio.sleep(self.__reset_delay)
            logger.info("Starting streamer ...")
            await self.__inner_start()

    @aiotools.atomic_fg
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

    # =====

    def set_params(self, params: dict) -> None:
        assert not self.__streamer_task
        return self.__params.set_params(params)

    def get_params(self) -> dict:
        return self.__params.get_params()

    # =====

    async def get_state(self) -> dict:
        streamer_state = None
        if self.__streamer_task:
            session = self.__ensure_http_session()
            try:
                async with session.get(self.__make_url("state")) as response:
                    htclient.raise_not_200(response)
                    streamer_state = (await response.json())["result"]
            except (aiohttp.ClientConnectionError, aiohttp.ServerConnectionError):
                pass
            except Exception:
                get_logger().exception("Invalid streamer response from /state")

        snapshot: (dict | None) = None
        if self.__snapshot:
            snapshot = dataclasses.asdict(self.__snapshot)
            del snapshot["headers"]
            del snapshot["data"]

        return {
            "limits": self.__params.get_limits(),
            "params": self.__params.get_params(),
            "snapshot": {"saved": snapshot},
            "streamer": streamer_state,
            "features": self.__params.get_features(),
        }

    async def poll_state(self) -> AsyncGenerator[dict, None]:
        def signal_handler(*_: Any) -> None:
            get_logger(0).info("Got SIGUSR2, checking the stream state ...")
            self.__notifier.notify()

        get_logger(0).info("Installing SIGUSR2 streamer handler ...")
        asyncio.get_event_loop().add_signal_handler(signal.SIGUSR2, signal_handler)

        waiter_task: (asyncio.Task | None) = None
        prev_state: dict = {}
        while True:
            state = await self.get_state()
            if state != prev_state:
                yield state
                prev_state = state

            if waiter_task is None:
                waiter_task = asyncio.create_task(self.__notifier.wait())
            if waiter_task in (await aiotools.wait_first(
                asyncio.ensure_future(asyncio.sleep(self.__state_poll)),
                waiter_task,
            ))[0]:
                waiter_task = None

    # =====

    async def take_snapshot(self, save: bool, load: bool, allow_offline: bool) -> (StreamerSnapshot | None):
        if load:
            return self.__snapshot
        logger = get_logger()
        session = self.__ensure_http_session()
        try:
            async with session.get(
                self.__make_url("snapshot"),
                timeout=self.__snapshot_timeout,
            ) as response:

                htclient.raise_not_200(response)
                online = (response.headers["X-UStreamer-Online"] == "true")
                if online or allow_offline:
                    snapshot = StreamerSnapshot(
                        online=online,
                        width=int(response.headers["X-UStreamer-Width"]),
                        height=int(response.headers["X-UStreamer-Height"]),
                        headers=tuple(
                            (key, value)
                            for (key, value) in tools.sorted_kvs(dict(response.headers))
                            if key.lower().startswith("x-ustreamer-") or key.lower() in [
                                "x-timestamp",
                                "access-control-allow-origin",
                                "cache-control",
                                "pragma",
                                "expires",
                            ]
                        ),
                        data=bytes(await response.read()),
                    )
                    if save:
                        self.__snapshot = snapshot
                        self.__notifier.notify()
                    return snapshot
                logger.error("Stream is offline, no signal or so")

        except (aiohttp.ClientConnectionError, aiohttp.ServerConnectionError) as err:
            logger.error("Can't connect to streamer: %s", tools.efmt(err))
        except Exception:
            logger.exception("Invalid streamer response from /snapshot")
        return None

    def remove_snapshot(self) -> None:
        self.__snapshot = None

    # =====

    @aiotools.atomic_fg
    async def cleanup(self) -> None:
        await self.ensure_stop(immediately=True)
        if self.__http_session:
            await self.__http_session.close()
            self.__http_session = None

    # =====

    def __ensure_http_session(self) -> aiohttp.ClientSession:
        if not self.__http_session:
            kwargs: dict = {
                "headers": {"User-Agent": htclient.make_user_agent("KVMD")},
                "connector": aiohttp.UnixConnector(path=self.__unix_path),
                "timeout": aiohttp.ClientTimeout(total=self.__timeout),
            }
            self.__http_session = aiohttp.ClientSession(**kwargs)
        return self.__http_session

    def __make_url(self, handle: str) -> str:
        assert not handle.startswith("/"), handle
        return f"http://localhost:0/{handle}"

    # =====

    @aiotools.atomic_fg
    async def __inner_start(self) -> None:
        assert not self.__streamer_task
        await self.__run_hook("PRE-START-CMD", self.__pre_start_cmd)
        self.__streamer_task = asyncio.create_task(self.__streamer_task_loop())

    @aiotools.atomic_fg
    async def __inner_stop(self) -> None:
        assert self.__streamer_task
        self.__streamer_task.cancel()
        await asyncio.gather(self.__streamer_task, return_exceptions=True)
        await self.__kill_streamer_proc()
        await self.__run_hook("POST-STOP-CMD", self.__post_stop_cmd)
        self.__streamer_task = None

    # =====

    async def __streamer_task_loop(self) -> None:  # pylint: disable=too-many-branches
        logger = get_logger(0)
        while True:  # pylint: disable=too-many-nested-blocks
            try:
                await self.__start_streamer_proc()
                assert self.__streamer_proc is not None
                await aioproc.log_stdout_infinite(self.__streamer_proc, logger)
                raise RuntimeError("Streamer unexpectedly died")
            except asyncio.CancelledError:
                break
            except Exception:
                if self.__streamer_proc:
                    logger.exception("Unexpected streamer error: pid=%d", self.__streamer_proc.pid)
                else:
                    logger.exception("Can't start streamer")
                await self.__kill_streamer_proc()
                await asyncio.sleep(1)

    def __make_cmd(self, cmd: list[str]) -> list[str]:
        return [
            part.format(
                unix=self.__unix_path,
                process_name_prefix=self.__process_name_prefix,
                **self.__params.get_params(),
            )
            for part in cmd
        ]

    async def __run_hook(self, name: str, cmd: list[str]) -> None:
        logger = get_logger()
        cmd = self.__make_cmd(cmd)
        logger.info("%s: %s", name, tools.cmdfmt(cmd))
        try:
            await aioproc.log_process(cmd, logger, prefix=name)
        except Exception as err:
            logger.exception("Can't execute command: %s", err)

    async def __start_streamer_proc(self) -> None:
        assert self.__streamer_proc is None
        cmd = self.__make_cmd(self.__cmd)
        self.__streamer_proc = await aioproc.run_process(cmd)
        get_logger(0).info("Started streamer pid=%d: %s", self.__streamer_proc.pid, tools.cmdfmt(cmd))

    async def __kill_streamer_proc(self) -> None:
        if self.__streamer_proc:
            await aioproc.kill_process(self.__streamer_proc, 1, get_logger(0))
        self.__streamer_proc = None
