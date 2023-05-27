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


import os
import asyncio

from typing import Callable
from typing import Any

from ...logging import get_logger

from ...inotify import InotifyMask
from ...inotify import Inotify

from ... import aiotools
from ... import usb

from ...yamlconf import Section

from ...validators.basic import valid_stripped_string_not_empty

from . import BaseUserGpioDriver


# =====
class Plugin(BaseUserGpioDriver):
    def __init__(
        self,
        instance_name: str,
        notifier: aiotools.AioNotifier,

        otg_config: Section,  # XXX: Not from options, see /kvmd/apps/kvmd/__init__.py for details
    ) -> None:

        super().__init__(instance_name, notifier)

        self.__udc: str = otg_config.udc
        self.__init_delay: float = otg_config.init_delay

        gadget: str = otg_config.gadget
        self.__udc_path = usb.get_gadget_path(gadget, usb.G_UDC)
        self.__functions_path = usb.get_gadget_path(gadget, usb.G_FUNCTIONS)
        self.__profile_path = usb.get_gadget_path(gadget, usb.G_PROFILE)

        self.__lock = asyncio.Lock()

    @classmethod
    def get_pin_validator(cls) -> Callable[[Any], Any]:
        return valid_stripped_string_not_empty

    def prepare(self) -> None:
        self.__udc = usb.find_udc(self.__udc)
        get_logger().info("Using UDC %s", self.__udc)

    async def run(self) -> None:
        logger = get_logger(0)
        while True:
            try:
                while True:
                    self._notifier.notify()
                    if os.path.isfile(self.__udc_path):
                        break
                    await asyncio.sleep(5)

                with Inotify() as inotify:
                    await inotify.watch(InotifyMask.ALL_MODIFY_EVENTS, os.path.dirname(self.__udc_path))
                    await inotify.watch(InotifyMask.ALL_MODIFY_EVENTS, self.__profile_path)
                    self._notifier.notify()
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
                            self._notifier.notify()
            except Exception:
                logger.exception("Unexpected OTG-bind watcher error")
                await asyncio.sleep(1)

    async def read(self, pin: str) -> bool:
        if pin == "udc":
            return self.__is_udc_enabled()
        return os.path.exists(os.path.join(self.__profile_path, pin))

    async def write(self, pin: str, state: bool) -> None:
        async with self.__lock:
            if pin == "udc":
                self.__set_udc_enabled(state)
            else:
                if self.__is_udc_enabled():
                    self.__set_udc_enabled(False)
                try:
                    if state:
                        os.symlink(
                            os.path.join(self.__functions_path, pin),
                            os.path.join(self.__profile_path, pin),
                        )
                    else:
                        os.unlink(os.path.join(self.__profile_path, pin))
                finally:
                    try:
                        await asyncio.sleep(self.__init_delay)
                    finally:
                        self.__set_udc_enabled(True)

    def __set_udc_enabled(self, enabled: bool) -> None:
        with open(self.__udc_path, "w") as file:
            file.write(self.__udc if enabled else "\n")

    def __is_udc_enabled(self) -> bool:
        with open(self.__udc_path) as file:
            return bool(file.read().strip())

    def __str__(self) -> str:
        return f"GPIO({self._instance_name})"

    __repr__ = __str__
