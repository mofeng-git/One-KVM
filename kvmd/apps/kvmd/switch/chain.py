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


import multiprocessing
import queue
import select
import dataclasses
import time

from typing import AsyncGenerator

from .lib import get_logger
from .lib import tools
from .lib import aiotools
from .lib import aioproc

from .types import Edids
from .types import Colors

from .proto import Response
from .proto import UnitState
from .proto import UnitAtxLeds

from .device import Device
from .device import DeviceError


# =====
class _BaseCmd:
    pass


@dataclasses.dataclass(frozen=True)
class _CmdSetActual(_BaseCmd):
    actual: bool


@dataclasses.dataclass(frozen=True)
class _CmdSetActivePort(_BaseCmd):
    port: int

    def __post_init__(self) -> None:
        assert self.port >= 0


@dataclasses.dataclass(frozen=True)
class _CmdSetPortBeacon(_BaseCmd):
    port: int
    on:   bool


@dataclasses.dataclass(frozen=True)
class _CmdSetUnitBeacon(_BaseCmd):
    unit:     int
    on:       bool
    downlink: bool


@dataclasses.dataclass(frozen=True)
class _CmdSetEdids(_BaseCmd):
    edids: Edids


@dataclasses.dataclass(frozen=True)
class _CmdSetColors(_BaseCmd):
    colors: Colors


@dataclasses.dataclass(frozen=True)
class _CmdAtxClick(_BaseCmd):
    port:       int
    delay:      float
    reset:      bool
    if_powered: (bool | None)

    def __post_init__(self) -> None:
        assert self.port >= 0
        assert 0.001 <= self.delay <= (0xFFFF / 1000)


@dataclasses.dataclass(frozen=True)
class _CmdRebootUnit(_BaseCmd):
    unit:       int
    bootloader: bool

    def __post_init__(self) -> None:
        assert self.unit >= 0


class _UnitContext:
    __TIMEOUT = 5.0

    def __init__(self) -> None:
        self.state:    (UnitState | None) = None
        self.atx_leds: (UnitAtxLeds | None) = None
        self.__rid = -1
        self.__deadline_ts = -1.0

    def can_be_changed(self) -> bool:
        return (
            self.state is not None
            and not self.state.flags.changing_busy
            and self.changing_rid < 0
        )

    # =====

    @property
    def changing_rid(self) -> int:
        if self.__deadline_ts >= 0 and self.__deadline_ts < time.monotonic():
            self.__rid = -1
            self.__deadline_ts = -1
        return self.__rid

    @changing_rid.setter
    def changing_rid(self, rid: int) -> None:
        self.__rid = rid
        self.__deadline_ts = ((time.monotonic() + self.__TIMEOUT) if rid >= 0 else -1)

    # =====

    def is_atx_allowed(self, ch: int) -> tuple[bool, bool]:  # (allowed, power_led)
        if self.state is None or self.atx_leds is None:
            return (False, False)
        return ((not self.state.atx_busy[ch]), self.atx_leds.power[ch])


# =====
class BaseEvent:
    pass


class DeviceFoundEvent(BaseEvent):
    pass


@dataclasses.dataclass(frozen=True)
class ChainTruncatedEvent(BaseEvent):
    units: int


@dataclasses.dataclass(frozen=True)
class PortActivatedEvent(BaseEvent):
    port: int


@dataclasses.dataclass(frozen=True)
class UnitStateEvent(BaseEvent):
    unit: int
    state: UnitState


@dataclasses.dataclass(frozen=True)
class UnitAtxLedsEvent(BaseEvent):
    unit: int
    atx_leds: UnitAtxLeds


# =====
class Chain:  # pylint: disable=too-many-instance-attributes
    def __init__(
        self,
        device_path: str,
        ignore_hpd_on_top: bool,
    ) -> None:

        self.__device = Device(device_path)
        self.__ignore_hpd_on_top = ignore_hpd_on_top

        self.__actual = False

        self.__edids = Edids()

        self.__colors = Colors()

        self.__units: list[_UnitContext] = []
        self.__active_port = -1

        self.__cmd_queue: "multiprocessing.Queue[_BaseCmd]" = multiprocessing.Queue()
        self.__events_queue: "multiprocessing.Queue[BaseEvent]" = multiprocessing.Queue()

        self.__stop_event = multiprocessing.Event()

    def set_actual(self, actual: bool) -> None:
        # Флаг разрешения синхронизации EDID и прочих чувствительных вещей
        self.__queue_cmd(_CmdSetActual(actual))

    # =====

    def set_active_port(self, port: int) -> None:
        self.__queue_cmd(_CmdSetActivePort(port))

    # =====

    def set_port_beacon(self, port: int, on: bool) -> None:
        self.__queue_cmd(_CmdSetPortBeacon(port, on))

    def set_uplink_beacon(self, unit: int, on: bool) -> None:
        self.__queue_cmd(_CmdSetUnitBeacon(unit, on, downlink=False))

    def set_downlink_beacon(self, unit: int, on: bool) -> None:
        self.__queue_cmd(_CmdSetUnitBeacon(unit, on, downlink=True))

    # =====

    def set_edids(self, edids: Edids) -> None:
        self.__queue_cmd(_CmdSetEdids(edids))  # Will be copied because of multiprocessing.Queue()

    def set_colors(self, colors: Colors) -> None:
        self.__queue_cmd(_CmdSetColors(colors))

    # =====

    def click_power(self, port: int, delay: float, if_powered: (bool | None)) -> None:
        self.__queue_cmd(_CmdAtxClick(port, delay, reset=False, if_powered=if_powered))

    def click_reset(self, port: int, delay: float, if_powered: (bool | None)) -> None:
        self.__queue_cmd(_CmdAtxClick(port, delay, reset=True, if_powered=if_powered))

    # =====

    def reboot_unit(self, unit: int, bootloader: bool) -> None:
        self.__queue_cmd(_CmdRebootUnit(unit, bootloader))

    # =====

    async def poll_events(self) -> AsyncGenerator[BaseEvent, None]:
        proc = multiprocessing.Process(target=self.__subprocess, daemon=True)
        try:
            proc.start()
            while True:
                try:
                    yield (await aiotools.run_async(self.__events_queue.get, True, 0.1))
                except queue.Empty:
                    pass
        finally:
            if proc.is_alive():
                self.__stop_event.set()
            if proc.is_alive() or proc.exitcode is not None:
                await aiotools.run_async(proc.join)

    # =====

    def __queue_cmd(self, cmd: _BaseCmd) -> None:
        if not self.__stop_event.is_set():
            self.__cmd_queue.put_nowait(cmd)

    def __queue_event(self, event: BaseEvent) -> None:
        if not self.__stop_event.is_set():
            self.__events_queue.put_nowait(event)

    def __subprocess(self) -> None:
        logger = aioproc.settle("Switch", "switch")
        no_device_reported = False
        while True:
            try:
                if self.__device.has_device():
                    no_device_reported = False
                    with self.__device:
                        logger.info("Switch found")
                        self.__queue_event(DeviceFoundEvent())
                        self.__main_loop()
                elif not no_device_reported:
                    self.__queue_event(ChainTruncatedEvent(0))
                    logger.info("Switch is missing")
                    no_device_reported = True
            except DeviceError as ex:
                logger.error("%s", tools.efmt(ex))
            except Exception:
                logger.exception("Unexpected error in the Switch loop")
            tools.clear_queue(self.__cmd_queue)
            if self.__stop_event.is_set():
                break
            time.sleep(1)

    def __main_loop(self) -> None:
        self.__device.request_state()
        self.__device.request_atx_leds()
        while not self.__stop_event.is_set():
            if self.__select():
                for resp in self.__device.read_all():
                    self.__update_units(resp)
                    self.__adjust_quirks()
                    self.__adjust_start_port()
                    self.__finish_changing_request(resp)
                self.__consume_commands()
            self.__ensure_config()

    def __select(self) -> bool:
        try:
            return bool(select.select([
                self.__device.get_fd(),
                self.__cmd_queue._reader,  # type: ignore  # pylint: disable=protected-access
            ], [], [], 1)[0])
        except Exception as ex:
            raise DeviceError(ex)

    def __consume_commands(self) -> None:
        while not self.__cmd_queue.empty():
            cmd = self.__cmd_queue.get()
            match cmd:
                case _CmdSetActual():
                    self.__actual = cmd.actual

                case _CmdSetActivePort():
                    # Может быть вызвано изнутри при синхронизации
                    self.__active_port = cmd.port
                    self.__queue_event(PortActivatedEvent(self.__active_port))

                case _CmdSetPortBeacon():
                    (unit, ch) = self.get_real_unit_channel(cmd.port)
                    self.__device.request_beacon(unit, ch, cmd.on)

                case _CmdSetUnitBeacon():
                    ch = (4 if cmd.downlink else 5)
                    self.__device.request_beacon(cmd.unit, ch, cmd.on)

                case _CmdAtxClick():
                    (unit, ch) = self.get_real_unit_channel(cmd.port)
                    if unit < len(self.__units):
                        (allowed, powered) = self.__units[unit].is_atx_allowed(ch)
                        if allowed and (cmd.if_powered is None or cmd.if_powered == powered):
                            delay_ms = min(int(cmd.delay * 1000), 0xFFFF)
                            if cmd.reset:
                                self.__device.request_atx_cr(unit, ch, delay_ms)
                            else:
                                self.__device.request_atx_cp(unit, ch, delay_ms)

                case _CmdSetEdids():
                    self.__edids = cmd.edids

                case _CmdSetColors():
                    self.__colors = cmd.colors

                case _CmdRebootUnit():
                    self.__device.request_reboot(cmd.unit, cmd.bootloader)

    def __update_units(self, resp: Response) -> None:
        units = resp.header.unit + 1
        while len(self.__units) < units:
            self.__units.append(_UnitContext())

        match resp.body:
            case UnitState():
                if not resp.body.flags.has_downlink and len(self.__units) > units:
                    del self.__units[units:]
                    self.__queue_event(ChainTruncatedEvent(units))
                self.__units[resp.header.unit].state = resp.body
                self.__queue_event(UnitStateEvent(resp.header.unit, resp.body))

            case UnitAtxLeds():
                self.__units[resp.header.unit].atx_leds = resp.body
                self.__queue_event(UnitAtxLedsEvent(resp.header.unit, resp.body))

    def __adjust_quirks(self) -> None:
        for (unit, ctx) in enumerate(self.__units):
            if ctx.state is not None and (ctx.state.version.sw_dev or ctx.state.version.sw >= 7):
                ignore_hpd = (unit == 0 and self.__ignore_hpd_on_top)
                if ctx.state.quirks.ignore_hpd != ignore_hpd:
                    get_logger().info("Applying quirk ignore_hpd=%s to [%d] ...",
                                      ignore_hpd, unit)
                    self.__device.request_set_quirks(unit, ignore_hpd)

    def __adjust_start_port(self) -> None:
        if self.__active_port < 0:
            for (unit, ctx) in enumerate(self.__units):
                if ctx.state is not None and ctx.state.ch < 4:
                    # Trigger queue select()
                    port = self.get_virtual_port(unit, ctx.state.ch)
                    get_logger().info("Found an active port %d on [%d:%d]: Syncing ...",
                                      port, unit, ctx.state.ch)
                    self.set_active_port(port)
                    break

    def __finish_changing_request(self, resp: Response) -> None:
        if self.__units[resp.header.unit].changing_rid == resp.header.rid:
            self.__units[resp.header.unit].changing_rid = -1

    # =====

    def __ensure_config(self) -> None:
        for (unit, ctx) in enumerate(self.__units):
            if ctx.state is not None:
                self.__ensure_config_port(unit, ctx)
                if self.__actual:
                    self.__ensure_config_edids(unit, ctx)
                    self.__ensure_config_colors(unit, ctx)

    def __ensure_config_port(self, unit: int, ctx: _UnitContext) -> None:
        assert ctx.state is not None
        if self.__active_port >= 0 and ctx.can_be_changed():
            ch = self.get_unit_target_channel(unit, self.__active_port)
            if ctx.state.ch != ch:
                get_logger().info("Switching for active port %d: [%d:%d] -> [%d:%d] ...",
                                  self.__active_port, unit, ctx.state.ch, unit, ch)
                ctx.changing_rid = self.__device.request_switch(unit, ch)

    def __ensure_config_edids(self, unit: int, ctx: _UnitContext) -> None:
        assert self.__actual
        assert ctx.state is not None
        if ctx.can_be_changed():
            for ch in range(4):
                port = self.get_virtual_port(unit, ch)
                edid = self.__edids.get_edid_for_port(port)
                if not ctx.state.compare_edid(ch, edid):
                    get_logger().info("Changing EDID on port %d on [%d:%d]: %d/%d -> %d/%d (%s) ...",
                                      port, unit, ch,
                                      ctx.state.video_crc[ch], ctx.state.video_edid[ch],
                                      edid.crc, edid.valid, edid.name)
                    ctx.changing_rid = self.__device.request_set_edid(unit, ch, edid)
                    break  # Busy globally

    def __ensure_config_colors(self, unit: int, ctx: _UnitContext) -> None:
        assert self.__actual
        assert ctx.state is not None
        for np in range(6):
            if self.__colors.crc != ctx.state.np_crc[np]:
                # get_logger().info("Changing colors on NP [%d:%d]: %d -> %d ...",
                #                   unit, np, ctx.state.np_crc[np], self.__colors.crc)
                self.__device.request_set_colors(unit, np, self.__colors)

    # =====

    @classmethod
    def get_real_unit_channel(cls, port: int) -> tuple[int, int]:
        return (port // 4, port % 4)

    @classmethod
    def get_unit_target_channel(cls, unit: int, port: int) -> int:
        (t_unit, t_ch) = cls.get_real_unit_channel(port)
        if unit != t_unit:
            t_ch = 4
        return t_ch

    @classmethod
    def get_virtual_port(cls, unit: int, ch: int) -> int:
        return (unit * 4) + ch
