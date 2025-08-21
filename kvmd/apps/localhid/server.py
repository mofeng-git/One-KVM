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
import errno

from typing import Callable
from typing import Coroutine

import aiohttp
import async_lru

from evdev import ecodes

from ...logging import get_logger

from ... import tools
from ... import aiotools

from ...keyboard.magic import MagicHandler

from ...clients.kvmd import KvmdClient
from ...clients.kvmd import KvmdClientSession
from ...clients.kvmd import KvmdClientWs

from .hid import Hid
from .multi import MultiHid


# =====
class LocalHidServer:  # pylint: disable=too-many-instance-attributes
    def __init__(self, kvmd: KvmdClient) -> None:
        self.__kvmd = kvmd

        self.__kvmd_session: (KvmdClientSession | None) = None
        self.__kvmd_ws: (KvmdClientWs | None) = None

        self.__queue: asyncio.Queue[tuple[int, tuple]] = asyncio.Queue()
        self.__hid = MultiHid(self.__queue)

        self.__info_switch_units = 0
        self.__info_switch_active = ""
        self.__info_mouse_absolute = True
        self.__info_mouse_outputs: list[str] = []

        self.__magic = MagicHandler(
            proxy_handler=self.__on_magic_key_proxy,
            key_handlers={
                ecodes.KEY_H:     self.__on_magic_grab,
                ecodes.KEY_K:     self.__on_magic_ungrab,
                ecodes.KEY_UP:    self.__on_magic_switch_prev,
                ecodes.KEY_LEFT:  self.__on_magic_switch_prev,
                ecodes.KEY_DOWN:  self.__on_magic_switch_next,
                ecodes.KEY_RIGHT: self.__on_magic_switch_next,
            },
            numeric_handler=self.__on_magic_switch_port,
        )

    def run(self) -> None:
        try:
            aiotools.run(self.__inner_run())
        finally:
            get_logger(0).info("Bye-bye")

    async def __inner_run(self) -> None:
        await aiotools.spawn_and_follow(
            self.__create_loop(self.__hid.run),
            self.__create_loop(self.__queue_worker),
            self.__create_loop(self.__api_worker),
        )

    async def __create_loop(self, func: Callable[[], Coroutine]) -> None:
        while True:
            try:
                await func()
            except Exception as ex:
                if isinstance(ex, OSError) and ex.errno == errno.ENODEV:  # pylint: disable=no-member
                    pass  # Device disconnected
                elif isinstance(ex, aiohttp.ClientError):
                    get_logger(0).error("KVMD client error: %s", tools.efmt(ex))
                else:
                    get_logger(0).exception("Unhandled exception in the loop: %s", func)
                await asyncio.sleep(5)

    async def __queue_worker(self) -> None:
        while True:
            (event, args) = await self.__queue.get()
            if event == Hid.KEY:
                await self.__magic.handle_key(*args)
                continue
            elif self.__hid.is_grabbed() and self.__kvmd_session and self.__kvmd_ws:
                match event:
                    case Hid.MOUSE_BUTTON:
                        await self.__kvmd_ws.send_mouse_button_event(*args)
                    case Hid.MOUSE_REL:
                        await self.__ensure_mouse_relative()
                        await self.__kvmd_ws.send_mouse_relative_event(*args)
                    case Hid.MOUSE_WHEEL:
                        await self.__kvmd_ws.send_mouse_wheel_event(*args)

    async def __api_worker(self) -> None:
        logger = get_logger(0)
        async with self.__kvmd.make_session() as session:
            async with session.ws(stream=False) as ws:
                logger.info("KVMD session opened")
                self.__kvmd_session = session
                self.__kvmd_ws = ws
                try:
                    async for (event_type, event) in ws.communicate():
                        if event_type == "hid":
                            if "leds" in event.get("keyboard", {}):
                                await self.__hid.set_leds(**event["keyboard"]["leds"])
                            if "absolute" in event.get("mouse", {}):
                                self.__info_mouse_outputs = event["mouse"]["outputs"]["available"]
                                self.__info_mouse_absolute = event["mouse"]["absolute"]
                        elif event_type == "switch":
                            if "model" in event:
                                self.__info_switch_units = len(event["model"]["units"])
                            if "summary" in event:
                                self.__info_switch_active = event["summary"]["active_id"]
                finally:
                    logger.info("KVMD session closed")
                    self.__kvmd_session = None
                    self.__kvmd_ws = None

    # =====

    async def __ensure_mouse_relative(self) -> None:
        if self.__info_mouse_absolute:
            # Avoid unnecessary LRU checks, just to speed up a bit
            await self.__inner_ensure_mouse_relative()

    @async_lru.alru_cache(maxsize=1, ttl=1)
    async def __inner_ensure_mouse_relative(self) -> None:
        if self.__kvmd_session and self.__info_mouse_absolute:
            for output in ["usb_rel", "ps2"]:
                if output in self.__info_mouse_outputs:
                    await self.__kvmd_session.hid.set_params(mouse_output=output)

    async def __on_magic_key_proxy(self, key: int, state: bool) -> None:
        if self.__hid.is_grabbed() and self.__kvmd_ws:
            await self.__kvmd_ws.send_key_event(key, state)

    async def __on_magic_grab(self) -> None:
        await self.__hid.set_grabbed(True)

    async def __on_magic_ungrab(self) -> None:
        await self.__hid.set_grabbed(False)

    async def __on_magic_switch_prev(self) -> None:
        if self.__kvmd_session and self.__info_switch_units > 0:
            get_logger(0).info("Switching port to the previous one ...")
            await self.__kvmd_session.switch.set_active_prev()

    async def __on_magic_switch_next(self) -> None:
        if self.__kvmd_session and self.__info_switch_units > 0:
            get_logger(0).info("Switching port to the next one ...")
            await self.__kvmd_session.switch.set_active_next()

    async def __on_magic_switch_port(self, codes: list[int]) -> bool:
        assert len(codes) > 0
        if self.__info_switch_units <= 0:
            return True
        elif 1 <= self.__info_switch_units <= 2:
            port = float(codes[0])
        else:  # self.__info_switch_units > 2:
            if len(codes) == 1:
                return False  # Wait for the second key
            port = (codes[0] + 1) + (codes[1] + 1) / 10
        if self.__kvmd_session:
            get_logger(0).info("Switching port to %s ...", port)
            await self.__kvmd_session.switch.set_active(port)
        return True
