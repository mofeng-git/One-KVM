# ========================================================================== #
#                                                                            #
#    KVMD - The main PiKVM daemon.                                           #
#                                                                            #
#    Copyright (C) 2020  Maxim Devaev <mdevaev@gmail.com>                    #
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
import dataclasses
import errno

from typing import AsyncGenerator

import pyudev

from ...logging import get_logger

from ... import aiotools

from .hid import Hid


# =====
def _udev_check(device: pyudev.Device) -> str:
    props = device.properties
    if props.get("ID_INPUT") == "1":
        path = props.get("DEVNAME")
        if isinstance(path, str) and path.startswith("/dev/input/event"):
            return path
    return ""


async def _follow_udev_hids() -> AsyncGenerator[tuple[bool, str], None]:
    ctx = pyudev.Context()

    monitor = pyudev.Monitor.from_netlink(pyudev.Context())
    monitor.filter_by(subsystem="input")
    monitor.start()
    fd = monitor.fileno()

    read_event = asyncio.Event()
    loop = asyncio.get_event_loop()
    loop.add_reader(fd, read_event.set)

    try:
        for device in ctx.list_devices(subsystem="input"):
            path = _udev_check(device)
            if path:
                yield (True, path)

        while True:
            await read_event.wait()
            while True:
                device = monitor.poll(0)
                if device is None:
                    read_event.clear()
                    break
                path = _udev_check(device)
                if path:
                    if device.action == "add":
                        yield (True, path)
                    elif device.action == "remove":
                        yield (False, path)
    finally:
        loop.remove_reader(fd)


@dataclasses.dataclass
class _Worker:
    task: asyncio.Task
    hid:  (Hid | None)


class MultiHid:
    def __init__(self, queue: asyncio.Queue[tuple[int, tuple]]) -> None:
        self.__queue = queue
        self.__workers: dict[str, _Worker] = {}
        self.__grabbed = True
        self.__leds = (False, False, False)

    async def run(self) -> None:
        logger = get_logger(0)
        logger.info("Starting UDEV loop ...")
        try:
            async for (added, path) in _follow_udev_hids():
                if added:
                    await self.__add_worker(path)
                else:
                    await self.__remove_worker(path)
        finally:
            logger.info("Cleanup ...")
            await aiotools.shield_fg(self.__cleanup())

    async def __cleanup(self) -> None:
        for path in list(self.__workers):
            await self.__remove_worker(path)

    async def __add_worker(self, path: str) -> None:
        if path in self.__workers:
            await self.__remove_worker(path)
        self.__workers[path] = _Worker(asyncio.create_task(self.__worker_task_loop(path)), None)

    async def __remove_worker(self, path: str) -> None:
        if path not in self.__workers:
            return
        try:
            worker = self.__workers[path]
            worker.task.cancel()
            await asyncio.gather(worker.task, return_exceptions=True)
        except Exception:
            pass
        finally:
            self.__workers.pop(path, None)

    async def __worker_task_loop(self, path: str) -> None:
        logger = get_logger(0)
        while True:
            hid: (Hid | None) = None
            try:
                hid = Hid(path)
                if not hid.is_suitable():
                    break
                logger.info("Opened: %s", hid)
                if self.__grabbed:
                    hid.set_grabbed(True)
                    hid.set_leds(*self.__leds)
                self.__workers[path].hid = hid
                await hid.poll_to_queue(self.__queue)
            except Exception as ex:
                if isinstance(ex, OSError) and ex.errno == errno.ENODEV:  # pylint: disable=no-member
                    logger.info("Closed: %s", hid)
                    break
                logger.exception("Unhandled exception while polling %s", hid)
                await asyncio.sleep(5)
            finally:
                self.__workers[path].hid = None
                if hid:
                    hid.close()

    def is_grabbed(self) -> bool:
        return self.__grabbed

    async def set_grabbed(self, grabbed: bool) -> None:
        await aiotools.run_async(self.__inner_set_grabbed, grabbed)

    def __inner_set_grabbed(self, grabbed: bool) -> None:
        if self.__grabbed != grabbed:
            get_logger(0).info("Grabbing ..." if grabbed else "Ungrabbing ...")
        self.__grabbed = grabbed
        for worker in self.__workers.values():
            if worker.hid:
                worker.hid.set_grabbed(grabbed)
        self.__inner_set_leds(*self.__leds)

    async def set_leds(self, caps: bool, scroll: bool, num: bool) -> None:
        await aiotools.run_async(self.__inner_set_leds, caps, scroll, num)

    def __inner_set_leds(self, caps: bool, scroll: bool, num: bool) -> None:
        self.__leds = (caps, scroll, num)
        if self.__grabbed:
            for worker in self.__workers.values():
                if worker.hid:
                    worker.hid.set_leds(*self.__leds)
