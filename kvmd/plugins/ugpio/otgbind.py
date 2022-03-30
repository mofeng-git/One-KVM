# ========================================================================== #
#                                                                            #
#    KVMD - The main PiKVM daemon.                                           #
#                                                                            #
#    Copyright (C) 2018-2022  Maxim Devaev <mdevaev@gmail.com>               #
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
        gadget: str,  # ditto
    ) -> None:

        super().__init__(instance_name, notifier)

        self.__udc = udc

        self.__ctl_path = f"{env.SYSFS_PREFIX}/sys/kernel/config/usb_gadget/{gadget}/UDC"

    @classmethod
    def get_pin_validator(cls) -> Callable[[Any], Any]:
        return str

    def prepare(self) -> None:
        self.__udc = usb.find_udc(self.__udc)[0]
        get_logger().info("Using UDC %s", self.__udc)

    async def run(self) -> None:
        logger = get_logger(0)
        while True:
            try:
                while True:
                    await self._notifier.notify()
                    if os.path.isfile(self.__ctl_path):
                        break
                    await asyncio.sleep(5)

                with Inotify() as inotify:
                    inotify.watch(os.path.dirname(self.__ctl_path), InotifyMask.ALL_MODIFY_EVENTS)
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

    async def cleanup(self) -> None:
        with open(self.__ctl_path) as ctl_file:
            ctl_file.write(self.__udc)

    async def read(self, pin: str) -> bool:
        _ = pin
        with open(self.__ctl_path) as ctl_file:
            return bool(ctl_file.read().strip())

    async def write(self, pin: str, state: bool) -> None:
        _ = pin
        with open(self.__ctl_path, "w") as ctl_file:
            ctl_file.write(self.__udc if state else "\n")

    def __str__(self) -> str:
        return f"GPIO({self._instance_name})"

    __repr__ = __str__
