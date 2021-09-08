# ========================================================================== #
#                                                                            #
#    KVMD - The main PiKVM daemon.                                           #
#                                                                            #
#    Copyright (C) 2018-2021  Maxim Devaev <mdevaev@gmail.com>               #
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
import asyncio

from typing import Callable
from typing import Any

from ...logging import get_logger

from ...inotify import InotifyMask
from ...inotify import Inotify

from ... import env
from ... import aiotools
from ... import usb

from . import BaseUserGpioDriver


# =====
class Plugin(BaseUserGpioDriver):
    def __init__(
        self,
        instance_name: str,
        notifier: aiotools.AioNotifier,

        udc: str,  # XXX: Not from options, see /kvmd/apps/kvmd/__init__.py for details
    ) -> None:

        super().__init__(instance_name, notifier)

        self.__udc = udc
        self.__driver = ""

    @classmethod
    def get_pin_validator(cls) -> Callable[[Any], Any]:
        return str

    def prepare(self) -> None:
        (self.__udc, self.__driver) = usb.find_udc(self.__udc)
        get_logger().info("Using UDC %s", self.__udc)

    async def run(self) -> None:
        logger = get_logger(0)
        while True:
            try:
                while True:
                    await self._notifier.notify()
                    if os.path.isdir(self.__get_driver_path()):
                        break
                    await asyncio.sleep(5)

                with Inotify() as inotify:
                    inotify.watch(self.__get_driver_path(), InotifyMask.ALL_MODIFY_EVENTS)
                    await self._notifier.notify()
                    while True:
                        need_restart = False
                        need_notify = False
                        for event in (await inotify.get_series(timeout=1)):
                            need_notify = True
                            if event.mask & (InotifyMask.DELETE_SELF | InotifyMask.MOVE_SELF | InotifyMask.UNMOUNT):
                                logger.warning("Got fatal inotify event: %s; reinitializing OTG-bind ...", event)
                                need_restart = True
                                break
                        if need_restart:
                            break
                        if need_notify:
                            await self._notifier.notify()
            except Exception:
                logger.exception("Unexpected OTG-bind watcher error")

    async def read(self, pin: str) -> bool:
        _ = pin
        return os.path.islink(self.__get_driver_path(self.__udc))

    async def write(self, pin: str, state: bool) -> None:
        _ = pin
        with open(self.__get_driver_path("bind" if state else "unbind"), "w") as ctl_file:
            ctl_file.write(f"{self.__udc}\n")

    def __get_driver_path(self, name: str="") -> str:
        assert self.__driver
        path = f"{env.SYSFS_PREFIX}/sys/bus/platform/drivers/{self.__driver}"
        return (os.path.join(path, name) if name else path)

    def __str__(self) -> str:
        return f"GPIO({self._instance_name})"

    __repr__ = __str__
