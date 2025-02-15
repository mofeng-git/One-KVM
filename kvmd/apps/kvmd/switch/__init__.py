# ========================================================================== #
#                                                                            #
#    KVMD - The main PiKVM daemon.                                           #
#                                                                            #
#    Copyright (C) 2018-2024  Maxim Devaev <mdevaev@gmail.com>               #
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

from typing import AsyncGenerator

from .lib import OperationError
from .lib import get_logger
from .lib import aiotools
from .lib import Inotify

from .types import Edid
from .types import Edids
from .types import Color
from .types import Colors
from .types import PortNames
from .types import AtxClickPowerDelays
from .types import AtxClickPowerLongDelays
from .types import AtxClickResetDelays

from .chain import DeviceFoundEvent
from .chain import ChainTruncatedEvent
from .chain import PortActivatedEvent
from .chain import UnitStateEvent
from .chain import UnitAtxLedsEvent
from .chain import Chain

from .state import StateCache

from .storage import Storage


# =====
class SwitchError(Exception):
    pass


class SwitchOperationError(OperationError, SwitchError):
    pass


class SwitchUnknownEdidError(SwitchOperationError):
    def __init__(self) -> None:
        super().__init__("No specified EDID ID found")


# =====
class Switch:  # pylint: disable=too-many-public-methods
    __X_EDIDS          = "edids"
    __X_COLORS         = "colors"
    __X_PORT_NAMES     = "port_names"
    __X_ATX_CP_DELAYS  = "atx_cp_delays"
    __X_ATX_CPL_DELAYS = "atx_cpl_delays"
    __X_ATX_CR_DELAYS  = "atx_cr_delays"

    __X_ALL = frozenset([
        __X_EDIDS, __X_COLORS, __X_PORT_NAMES,
        __X_ATX_CP_DELAYS, __X_ATX_CPL_DELAYS, __X_ATX_CR_DELAYS,
    ])

    def __init__(
        self,
        device_path: str,
        default_edid_path: str,
        pst_unix_path: str,
        ignore_hpd_on_top: bool,
    ) -> None:

        self.__default_edid_path = default_edid_path

        self.__chain = Chain(device_path, ignore_hpd_on_top)
        self.__cache = StateCache()
        self.__storage = Storage(pst_unix_path)

        self.__lock = asyncio.Lock()

        self.__save_notifier = aiotools.AioNotifier()

    # =====

    def __x_set_edids(self, edids: Edids, save: bool=True) -> None:
        self.__chain.set_edids(edids)
        self.__cache.set_edids(edids)
        if save:
            self.__save_notifier.notify()

    def __x_set_colors(self, colors: Colors, save: bool=True) -> None:
        self.__chain.set_colors(colors)
        self.__cache.set_colors(colors)
        if save:
            self.__save_notifier.notify()

    def __x_set_port_names(self, port_names: PortNames, save: bool=True) -> None:
        self.__cache.set_port_names(port_names)
        if save:
            self.__save_notifier.notify()

    def __x_set_atx_cp_delays(self, delays: AtxClickPowerDelays, save: bool=True) -> None:
        self.__cache.set_atx_cp_delays(delays)
        if save:
            self.__save_notifier.notify()

    def __x_set_atx_cpl_delays(self, delays: AtxClickPowerLongDelays, save: bool=True) -> None:
        self.__cache.set_atx_cpl_delays(delays)
        if save:
            self.__save_notifier.notify()

    def __x_set_atx_cr_delays(self, delays: AtxClickResetDelays, save: bool=True) -> None:
        self.__cache.set_atx_cr_delays(delays)
        if save:
            self.__save_notifier.notify()

    # =====

    async def set_active_port(self, port: int) -> None:
        self.__chain.set_active_port(port)

    # =====

    async def set_port_beacon(self, port: int, on: bool) -> None:
        self.__chain.set_port_beacon(port, on)

    async def set_uplink_beacon(self, unit: int, on: bool) -> None:
        self.__chain.set_uplink_beacon(unit, on)

    async def set_downlink_beacon(self, unit: int, on: bool) -> None:
        self.__chain.set_downlink_beacon(unit, on)

    # =====

    async def atx_power_on(self, port: int) -> None:
        self.__inner_atx_cp(port, False, self.__X_ATX_CP_DELAYS)

    async def atx_power_off(self, port: int) -> None:
        self.__inner_atx_cp(port, True, self.__X_ATX_CP_DELAYS)

    async def atx_power_off_hard(self, port: int) -> None:
        self.__inner_atx_cp(port, True, self.__X_ATX_CPL_DELAYS)

    async def atx_power_reset_hard(self, port: int) -> None:
        self.__inner_atx_cr(port, True)

    async def atx_click_power(self, port: int) -> None:
        self.__inner_atx_cp(port, None, self.__X_ATX_CP_DELAYS)

    async def atx_click_power_long(self, port: int) -> None:
        self.__inner_atx_cp(port, None, self.__X_ATX_CPL_DELAYS)

    async def atx_click_reset(self, port: int) -> None:
        self.__inner_atx_cr(port, None)

    def __inner_atx_cp(self, port: int, if_powered: (bool | None), x_delay: str) -> None:
        assert x_delay in [self.__X_ATX_CP_DELAYS, self.__X_ATX_CPL_DELAYS]
        delay = getattr(self.__cache, f"get_{x_delay}")()[port]
        self.__chain.click_power(port, delay, if_powered)

    def __inner_atx_cr(self, port: int, if_powered: (bool | None)) -> None:
        delay = self.__cache.get_atx_cr_delays()[port]
        self.__chain.click_reset(port, delay, if_powered)

    # =====

    async def create_edid(self, name: str, data_hex: str) -> str:
        async with self.__lock:
            edids = self.__cache.get_edids()
            edid_id = edids.add(Edid.from_data(name, data_hex))
            self.__x_set_edids(edids)
        return edid_id

    async def change_edid(
        self,
        edid_id: str,
        name: (str | None)=None,
        data_hex: (str | None)=None,
    ) -> None:

        assert edid_id != Edids.DEFAULT_ID
        async with self.__lock:
            edids = self.__cache.get_edids()
            if not edids.has_id(edid_id):
                raise SwitchUnknownEdidError()
            old = edids.get(edid_id)
            name = (name or old.name)
            data_hex = (data_hex or old.as_text())
            edids.set(edid_id, Edid.from_data(name, data_hex))
            self.__x_set_edids(edids)

    async def remove_edid(self, edid_id: str) -> None:
        assert edid_id != Edids.DEFAULT_ID
        async with self.__lock:
            edids = self.__cache.get_edids()
            if not edids.has_id(edid_id):
                raise SwitchUnknownEdidError()
            edids.remove(edid_id)
            self.__x_set_edids(edids)

    # =====

    async def set_colors(self, **values: str) -> None:
        async with self.__lock:
            old = self.__cache.get_colors()
            new = {}
            for role in Colors.ROLES:
                if role in values:
                    if values[role] != "default":
                        new[role] = Color.from_text(values[role])
                    # else reset to default
                else:
                    new[role] = getattr(old, role)
            self.__x_set_colors(Colors(**new))  # type: ignore

    # =====

    async def set_port_params(
        self,
        port: int,
        edid_id: (str | None)=None,
        name: (str | None)=None,
        atx_click_power_delay: (float | None)=None,
        atx_click_power_long_delay: (float | None)=None,
        atx_click_reset_delay: (float | None)=None,
    ) -> None:

        async with self.__lock:
            if edid_id is not None:
                edids = self.__cache.get_edids()
                if not edids.has_id(edid_id):
                    raise SwitchUnknownEdidError()
                edids.assign(port, edid_id)
                self.__x_set_edids(edids)

            for (key, value) in [
                (self.__X_PORT_NAMES,     name),
                (self.__X_ATX_CP_DELAYS,  atx_click_power_delay),
                (self.__X_ATX_CPL_DELAYS, atx_click_power_long_delay),
                (self.__X_ATX_CR_DELAYS,  atx_click_reset_delay),
            ]:
                if value is not None:
                    new = getattr(self.__cache, f"get_{key}")()
                    new[port] = (value or None)  # None == reset to default
                    getattr(self, f"_Switch__x_set_{key}")(new)

    # =====

    async def reboot_unit(self, unit: int, bootloader: bool) -> None:
        self.__chain.reboot_unit(unit, bootloader)

    # =====

    async def get_state(self) -> dict:
        return self.__cache.get_state()

    async def trigger_state(self) -> None:
        await self.__cache.trigger_state()

    async def poll_state(self) -> AsyncGenerator[dict, None]:
        async for state in self.__cache.poll_state():
            yield state

    # =====

    async def systask(self) -> None:
        tasks = [
            asyncio.create_task(self.__systask_events()),
            asyncio.create_task(self.__systask_default_edid()),
            asyncio.create_task(self.__systask_storage()),
        ]
        try:
            await asyncio.gather(*tasks)
        except Exception:
            for task in tasks:
                task.cancel()
            await asyncio.gather(*tasks, return_exceptions=True)
            raise

    async def __systask_events(self) -> None:
        async for event in self.__chain.poll_events():
            match event:
                case DeviceFoundEvent():
                    await self.__load_configs()
                case ChainTruncatedEvent():
                    self.__cache.truncate(event.units)
                case PortActivatedEvent():
                    self.__cache.update_active_port(event.port)
                case UnitStateEvent():
                    self.__cache.update_unit_state(event.unit, event.state)
                case UnitAtxLedsEvent():
                    self.__cache.update_unit_atx_leds(event.unit, event.atx_leds)

    async def __load_configs(self) -> None:
        async with self.__lock:
            try:
                async with self.__storage.readable() as ctx:
                    values = {
                        key: await getattr(ctx, f"read_{key}")()
                        for key in self.__X_ALL
                    }
                    data_hex = await aiotools.read_file(self.__default_edid_path)
                    values["edids"].set_default(data_hex)
            except Exception:
                get_logger(0).exception("Can't load configs")
            else:
                for (key, value) in values.items():
                    func = getattr(self, f"_Switch__x_set_{key}")
                    if isinstance(value, tuple):
                        func(*value, save=False)
                    else:
                        func(value, save=False)
                self.__chain.set_actual(True)

    async def __systask_default_edid(self) -> None:
        logger = get_logger(0)
        async for _ in self.__poll_default_edid():
            async with self.__lock:
                edids = self.__cache.get_edids()
                try:
                    data_hex = await aiotools.read_file(self.__default_edid_path)
                    edids.set_default(data_hex)
                except Exception:
                    logger.exception("Can't read default EDID, ignoring ...")
                else:
                    self.__x_set_edids(edids, save=False)

    async def __poll_default_edid(self) -> AsyncGenerator[None, None]:
        logger = get_logger(0)
        while True:
            while not os.path.exists(self.__default_edid_path):
                await asyncio.sleep(5)
            try:
                with Inotify() as inotify:
                    await inotify.watch_all_changes(self.__default_edid_path)
                    if os.path.islink(self.__default_edid_path):
                        await inotify.watch_all_changes(os.path.realpath(self.__default_edid_path))
                    yield None
                    while True:
                        need_restart = False
                        need_notify = False
                        for event in (await inotify.get_series(timeout=1)):
                            need_notify = True
                            if event.restart:
                                logger.warning("Got fatal inotify event: %s; reinitializing ...", event)
                                need_restart = True
                                break
                        if need_restart:
                            break
                        if need_notify:
                            yield None
            except Exception:
                logger.exception("Unexpected watcher error")
                await asyncio.sleep(1)

    async def __systask_storage(self) -> None:
        # При остановке KVMD можем не успеть записать, ну да пофиг
        prevs = dict.fromkeys(self.__X_ALL)
        while True:
            await self.__save_notifier.wait()
            while (await self.__save_notifier.wait(5)):
                pass
            while True:
                try:
                    async with self.__lock:
                        write = {
                            key: new
                            for (key, old) in prevs.items()
                            if (new := getattr(self.__cache, f"get_{key}")()) != old
                        }
                        if write:
                            async with self.__storage.writable() as ctx:
                                for (key, new) in write.items():
                                    func = getattr(ctx, f"write_{key}")
                                    if isinstance(new, tuple):
                                        await func(*new)
                                    else:
                                        await func(new)
                                    prevs[key] = new
                except Exception:
                    get_logger(0).exception("Unexpected storage error")
                    await asyncio.sleep(5)
                else:
                    break
