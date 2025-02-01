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


import asyncio
import dataclasses
import time

from typing import AsyncGenerator

from .types import Edids
from .types import Color
from .types import Colors
from .types import PortNames
from .types import AtxClickPowerDelays
from .types import AtxClickPowerLongDelays
from .types import AtxClickResetDelays

from .proto import UnitState
from .proto import UnitAtxLeds

from .chain import Chain


# =====
@dataclasses.dataclass
class _UnitInfo:
    state: (UnitState | None) = dataclasses.field(default=None)
    atx_leds: (UnitAtxLeds | None) = dataclasses.field(default=None)


# =====
class StateCache:  # pylint: disable=too-many-instance-attributes
    __FW_VERSION = 5

    __FULL    = 0xFFFF
    __SUMMARY = 0x01
    __EDIDS   = 0x02
    __COLORS  = 0x04
    __VIDEO   = 0x08
    __USB     = 0x10
    __BEACONS = 0x20
    __ATX     = 0x40

    def __init__(self) -> None:
        self.__edids = Edids()
        self.__colors = Colors()
        self.__port_names = PortNames({})
        self.__atx_cp_delays = AtxClickPowerDelays({})
        self.__atx_cpl_delays = AtxClickPowerLongDelays({})
        self.__atx_cr_delays = AtxClickResetDelays({})

        self.__units: list[_UnitInfo] = []
        self.__active_port = -1
        self.__synced = True

        self.__queue: "asyncio.Queue[int]" = asyncio.Queue()

    def get_edids(self) -> Edids:
        return self.__edids.copy()

    def get_colors(self) -> Colors:
        return self.__colors

    def get_port_names(self) -> PortNames:
        return self.__port_names.copy()

    def get_atx_cp_delays(self) -> AtxClickPowerDelays:
        return self.__atx_cp_delays.copy()

    def get_atx_cpl_delays(self) -> AtxClickPowerLongDelays:
        return self.__atx_cpl_delays.copy()

    def get_atx_cr_delays(self) -> AtxClickResetDelays:
        return self.__atx_cr_delays.copy()

    # =====

    def get_state(self) -> dict:
        return self.__inner_get_state(self.__FULL)

    async def trigger_state(self) -> None:
        self.__bump_state(self.__FULL)

    async def poll_state(self) -> AsyncGenerator[dict, None]:
        atx_ts: float = 0
        while True:
            try:
                mask = await asyncio.wait_for(self.__queue.get(), timeout=0.1)
            except TimeoutError:
                mask = 0

            if mask == self.__ATX:
                # Откладываем единичное новое событие ATX, чтобы аккумулировать с нескольких свичей
                if atx_ts == 0:
                    atx_ts = time.monotonic() + 0.2
                    continue
                elif atx_ts >= time.monotonic():
                    continue
                # ... Ну или разрешаем отправить, если оно уже достаточно мариновалось
            elif mask == 0 and atx_ts > time.monotonic():
                # Разрешаем отправить отложенное
                mask = self.__ATX
                atx_ts = 0
            elif mask & self.__ATX:
                # Комплексное событие всегда должно обрабатываться сразу
                atx_ts = 0

            if mask != 0:
                yield self.__inner_get_state(mask)

    def __inner_get_state(self, mask: int) -> dict:  # pylint: disable=too-many-branches,too-many-statements,too-many-locals
        assert mask != 0
        x_model = (mask == self.__FULL)
        x_summary = (mask & self.__SUMMARY)
        x_edids = (mask & self.__EDIDS)
        x_colors = (mask & self.__COLORS)
        x_video = (mask & self.__VIDEO)
        x_usb = (mask & self.__USB)
        x_beacons = (mask & self.__BEACONS)
        x_atx = (mask & self.__ATX)

        state: dict = {}
        if x_model:
            state["model"] = {
                "firmware": {"version": self.__FW_VERSION},
                "units": [],
                "ports": [],
                "limits": {
                    "atx": {
                        "click_delays": {
                            key: {"default": value, "min": 0, "max": 10}
                            for (key, value) in [
                                ("power",      self.__atx_cp_delays.default),
                                ("power_long", self.__atx_cpl_delays.default),
                                ("reset",      self.__atx_cr_delays.default),
                            ]
                        },
                    },
                },
            }
        if x_summary:
            state["summary"] = {"active_port": self.__active_port, "synced": self.__synced}
        if x_edids:
            state["edids"] = {
                "all": {
                    edid_id: {
                        "name": edid.name,
                        "data": edid.as_text(),
                        "parsed": (dataclasses.asdict(edid.info) if edid.info is not None else None),
                    }
                    for (edid_id, edid) in self.__edids.all.items()
                },
                "used": [],
            }
        if x_colors:
            state["colors"] = {
                role: {
                    comp: getattr(getattr(self.__colors, role), comp)
                    for comp in Color.COMPONENTS
                }
                for role in Colors.ROLES
            }
        if x_video:
            state["video"] = {"links": []}
        if x_usb:
            state["usb"] = {"links": []}
        if x_beacons:
            state["beacons"] = {"uplinks": [], "downlinks": [], "ports": []}
        if x_atx:
            state["atx"] = {"busy": [], "leds": {"power": [], "hdd": []}}

        if not self.__is_units_ready():
            return state

        for (unit, ui) in enumerate(self.__units):
            assert ui.state is not None
            assert ui.atx_leds is not None
            if x_model:
                state["model"]["units"].append({"firmware": {"version": ui.state.sw_version}})
            if x_video:
                state["video"]["links"].extend(ui.state.video_5v_sens[:4])
            if x_usb:
                state["usb"]["links"].extend(ui.state.usb_5v_sens)
            if x_beacons:
                state["beacons"]["uplinks"].append(ui.state.beacons[5])
                state["beacons"]["downlinks"].append(ui.state.beacons[4])
                state["beacons"]["ports"].extend(ui.state.beacons[:4])
            if x_atx:
                state["atx"]["busy"].extend(ui.state.atx_busy)
                state["atx"]["leds"]["power"].extend(ui.atx_leds.power)
                state["atx"]["leds"]["hdd"].extend(ui.atx_leds.hdd)
            if x_model or x_edids:
                for ch in range(4):
                    port = Chain.get_virtual_port(unit, ch)
                    if x_model:
                        state["model"]["ports"].append({
                            "unit": unit,
                            "channel": ch,
                            "name": self.__port_names[port],
                            "atx": {
                                "click_delays": {
                                    "power": self.__atx_cp_delays[port],
                                    "power_long": self.__atx_cpl_delays[port],
                                    "reset": self.__atx_cr_delays[port],
                                },
                            },
                        })
                    if x_edids:
                        state["edids"]["used"].append(self.__edids.get_id_for_port(port))
        return state

    def __inner_check_synced(self) -> bool:
        for (unit, ui) in enumerate(self.__units):
            if ui.state is None or ui.state.flags.changing_busy:
                return False
            if (
                self.__active_port >= 0
                and ui.state.ch != Chain.get_unit_target_channel(unit, self.__active_port)
            ):
                return False
            for ch in range(4):
                port = Chain.get_virtual_port(unit, ch)
                edid = self.__edids.get_edid_for_port(port)
                if not ui.state.compare_edid(ch, edid):
                    return False
            for ch in range(6):
                if ui.state.np_crc[ch] != self.__colors.crc:
                    return False
        return True

    def __recache_synced(self) -> bool:
        synced = self.__inner_check_synced()
        if self.__synced != synced:
            self.__synced = synced
            return True
        return False

    def truncate(self, units: int) -> None:
        if len(self.__units) > units:
            del self.__units[units:]
            self.__bump_state(self.__FULL)

    def update_active_port(self, port: int) -> None:
        changed = (self.__active_port != port)
        self.__active_port = port
        changed = (self.__recache_synced() or changed)
        if changed:
            self.__bump_state(self.__SUMMARY)

    def update_unit_state(self, unit: int, new: UnitState) -> None:
        ui = self.__ensure_unit(unit)
        (prev, ui.state) = (ui.state, new)
        if not self.__is_units_ready():
            return
        mask = 0
        if prev is None:
            mask = self.__FULL
        else:
            if self.__recache_synced():
                mask |= self.__SUMMARY
            if prev.video_5v_sens != new.video_5v_sens:
                mask |= self.__VIDEO
            if prev.usb_5v_sens != new.usb_5v_sens:
                mask |= self.__USB
            if prev.beacons != new.beacons:
                mask |= self.__BEACONS
            if prev.atx_busy != new.atx_busy:
                mask |= self.__ATX
        if mask:
            self.__bump_state(mask)

    def update_unit_atx_leds(self, unit: int, new: UnitAtxLeds) -> None:
        ui = self.__ensure_unit(unit)
        (prev, ui.atx_leds) = (ui.atx_leds, new)
        if not self.__is_units_ready():
            return
        if prev is None:
            self.__bump_state(self.__FULL)
        elif prev != new:
            self.__bump_state(self.__ATX)

    def __is_units_ready(self) -> bool:
        for ui in self.__units:
            if ui.state is None or ui.atx_leds is None:
                return False
        return True

    def __ensure_unit(self, unit: int) -> _UnitInfo:
        while len(self.__units) < unit + 1:
            self.__units.append(_UnitInfo())
        return self.__units[unit]

    def __bump_state(self, mask: int) -> None:
        assert mask != 0
        self.__queue.put_nowait(mask)

    # =====

    def set_edids(self, edids: Edids) -> None:
        changed = (
            self.__edids.all != edids.all
            or not self.__edids.compare_on_ports(edids, self.__get_ports())
        )
        self.__edids = edids.copy()
        if changed:
            self.__bump_state(self.__EDIDS)

    def set_colors(self, colors: Colors) -> None:
        changed = (self.__colors != colors)
        self.__colors = colors
        if changed:
            self.__bump_state(self.__COLORS)

    def set_port_names(self, port_names: PortNames) -> None:
        changed = (not self.__port_names.compare_on_ports(port_names, self.__get_ports()))
        self.__port_names = port_names.copy()
        if changed:
            self.__bump_state(self.__FULL)

    def set_atx_cp_delays(self, delays: AtxClickPowerDelays) -> None:
        changed = (not self.__atx_cp_delays.compare_on_ports(delays, self.__get_ports()))
        self.__atx_cp_delays = delays.copy()
        if changed:
            self.__bump_state(self.__FULL)

    def set_atx_cpl_delays(self, delays: AtxClickPowerLongDelays) -> None:
        changed = (not self.__atx_cpl_delays.compare_on_ports(delays, self.__get_ports()))
        self.__atx_cpl_delays = delays.copy()
        if changed:
            self.__bump_state(self.__FULL)

    def set_atx_cr_delays(self, delays: AtxClickResetDelays) -> None:
        changed = (not self.__atx_cr_delays.compare_on_ports(delays, self.__get_ports()))
        self.__atx_cr_delays = delays.copy()
        if changed:
            self.__bump_state(self.__FULL)

    def __get_ports(self) -> int:
        return (len(self.__units) * 4)
