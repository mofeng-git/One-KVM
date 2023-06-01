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


import asyncio

from typing import AsyncGenerator
from typing import Callable
from typing import Any

from ...logging import get_logger

from ...errors import IsBusyError

from ... import tools
from ... import aiotools

from ...plugins.ugpio import GpioError
from ...plugins.ugpio import GpioOperationError
from ...plugins.ugpio import GpioDriverOfflineError
from ...plugins.ugpio import UserGpioModes
from ...plugins.ugpio import BaseUserGpioDriver
from ...plugins.ugpio import get_ugpio_driver_class

from ...yamlconf import Section


# =====
class GpioChannelNotFoundError(GpioOperationError):
    def __init__(self) -> None:
        super().__init__("GPIO channel is not found")


class GpioSwitchNotSupported(GpioOperationError):
    def __init__(self) -> None:
        super().__init__("This GPIO channel does not support switching")


class GpioPulseNotSupported(GpioOperationError):
    def __init__(self) -> None:
        super().__init__("This GPIO channel does not support pulsing")


class GpioChannelIsBusyError(IsBusyError, GpioError):
    def __init__(self) -> None:
        super().__init__("Performing another GPIO operation on this channel, please try again later")


# =====
class _GpioInput:
    def __init__(
        self,
        channel: str,
        config: Section,
        driver: BaseUserGpioDriver,
    ) -> None:

        self.__channel = channel
        self.__pin: str = str(config.pin)
        self.__inverted: bool = config.inverted

        self.__driver = driver
        self.__driver.register_input(self.__pin, config.debounce)

    def get_scheme(self) -> dict:
        return {
            "hw": {
                "driver": self.__driver.get_instance_id(),
                "pin": self.__pin,
            },
        }

    async def get_state(self) -> dict:
        (online, state) = (True, False)
        try:
            state = (await self.__driver.read(self.__pin) ^ self.__inverted)
        except GpioDriverOfflineError:
            online = False
        return {
            "online": online,
            "state": state,
        }

    def __str__(self) -> str:
        return f"Input({self.__channel}, driver={self.__driver}, pin={self.__pin})"

    __repr__ = __str__


class _GpioOutput:  # pylint: disable=too-many-instance-attributes
    def __init__(
        self,
        channel: str,
        config: Section,
        driver: BaseUserGpioDriver,
        notifier: aiotools.AioNotifier,
    ) -> None:

        self.__channel = channel
        self.__pin: str = str(config.pin)
        self.__inverted: bool = config.inverted

        self.__switch: bool = config.switch

        self.__pulse_delay = 0.0
        self.__min_pulse_delay = 0.0
        self.__max_pulse_delay = 0.0
        if config.pulse.delay:
            assert config.pulse.max_delay > 0
            self.__pulse_delay = min(max(config.pulse.delay, config.pulse.min_delay), config.pulse.max_delay)
            self.__min_pulse_delay = config.pulse.min_delay
            self.__max_pulse_delay = config.pulse.max_delay

        self.__busy_delay: float = config.busy_delay

        self.__driver = driver
        self.__driver.register_output(self.__pin, (None if config.initial is None else (config.initial ^ config.inverted)))

        self.__region = aiotools.AioExclusiveRegion(GpioChannelIsBusyError, notifier)

    def is_const(self) -> bool:
        return (not self.__switch and not self.__pulse_delay)

    def get_scheme(self) -> dict:
        return {
            "switch": self.__switch,
            "pulse": {
                "delay": self.__pulse_delay,
                "min_delay": self.__min_pulse_delay,
                "max_delay": self.__max_pulse_delay,
            },
            "hw": {
                "driver": self.__driver.get_instance_id(),
                "pin": self.__pin,
            },
        }

    async def get_state(self) -> dict:
        busy = self.__region.is_busy()
        (online, state) = (True, False)
        if not busy:
            try:
                state = await self.__read()
            except GpioDriverOfflineError:
                online = False
        return {
            "online": online,
            "state": state,
            "busy": busy,
        }

    async def switch(self, state: bool, wait: bool) -> None:
        if not self.__switch:
            raise GpioSwitchNotSupported()
        await self.__run_action(wait, "switch", self.__inner_switch, state)

    @aiotools.atomic_fg
    async def pulse(self, delay: float, wait: bool) -> None:
        if not self.__pulse_delay:
            raise GpioPulseNotSupported()
        delay = min(max((delay or self.__pulse_delay), self.__min_pulse_delay), self.__max_pulse_delay)
        await self.__run_action(wait, "pulse", self.__inner_pulse, delay)

    # =====

    @aiotools.atomic_fg
    async def __run_action(self, wait: bool, name: str, func: Callable, *args: Any) -> None:
        if wait:
            async with self.__region:
                await func(*args)
        else:
            await aiotools.run_region_task(
                f"Can't perform {name} of {self} or operation was not completed",
                self.__region, self.__action_task_wrapper, name, func, *args,
            )

    @aiotools.atomic_fg
    async def __action_task_wrapper(self, name: str, func: Callable, *args: Any) -> None:
        try:
            return (await func(*args))
        except GpioDriverOfflineError:
            get_logger(0).error("Can't perform %s of %s or operation was not completed: driver offline", name, self)

    @aiotools.atomic_fg
    async def __inner_switch(self, state: bool) -> None:
        await self.__write(state)
        get_logger(0).info("Ensured switch %s to state=%d", self, state)
        await asyncio.sleep(self.__busy_delay)

    @aiotools.atomic_fg
    async def __inner_pulse(self, delay: float) -> None:
        try:
            await self.__write(True)
            await asyncio.sleep(delay)
        finally:
            await self.__write(False)
            await asyncio.sleep(self.__busy_delay)
        get_logger(0).info("Pulsed %s with delay=%.2f", self, delay)

    # =====

    async def __read(self) -> bool:
        return (await self.__driver.read(self.__pin) ^ self.__inverted)

    async def __write(self, state: bool) -> None:
        await self.__driver.write(self.__pin, (state ^ self.__inverted))

    def __str__(self) -> str:
        return f"Output({self.__channel}, driver={self.__driver}, pin={self.__pin})"

    __repr__ = __str__


# =====
class UserGpio:
    def __init__(self, config: Section, otg_config: Section) -> None:
        self.__view = config.view

        self.__notifier = aiotools.AioNotifier()

        self.__drivers = {
            driver: get_ugpio_driver_class(drv_config.type)(
                instance_name=driver,
                notifier=self.__notifier,
                **drv_config._unpack(ignore=["instance_name", "notifier", "type"]),
                **({"otg_config": otg_config} if drv_config.type == "otgconf" else {}),  # Hack
            )
            for (driver, drv_config) in tools.sorted_kvs(config.drivers)
        }

        self.__inputs: dict[str, _GpioInput] = {}
        self.__outputs: dict[str, _GpioOutput] = {}

        for (channel, ch_config) in tools.sorted_kvs(config.scheme):
            driver = self.__drivers[ch_config.driver]
            if ch_config.mode == UserGpioModes.INPUT:
                self.__inputs[channel] = _GpioInput(channel, ch_config, driver)
            else:  # output:
                self.__outputs[channel] = _GpioOutput(channel, ch_config, driver, self.__notifier)

    async def get_model(self) -> dict:
        return {
            "scheme": {
                "inputs": {channel: gin.get_scheme() for (channel, gin) in self.__inputs.items()},
                "outputs": {
                    channel: gout.get_scheme()
                    for (channel, gout) in self.__outputs.items()
                    if not gout.is_const()
                },
            },
            "view": self.__make_view(),
        }

    async def get_state(self) -> dict:
        return {
            "inputs": {channel: await gin.get_state() for (channel, gin) in self.__inputs.items()},
            "outputs": {
                channel: await gout.get_state()
                for (channel, gout) in self.__outputs.items()
                if not gout.is_const()
            },
        }

    async def poll_state(self) -> AsyncGenerator[dict, None]:
        prev_state: dict = {}
        while True:
            state = await self.get_state()
            if state != prev_state:
                yield state
                prev_state = state
            await self.__notifier.wait()

    def sysprep(self) -> None:
        get_logger(0).info("Preparing User-GPIO drivers ...")
        for (_, driver) in tools.sorted_kvs(self.__drivers):
            driver.prepare()

    async def systask(self) -> None:
        get_logger(0).info("Running User-GPIO drivers ...")
        await asyncio.gather(*[
            driver.run()
            for (_, driver) in tools.sorted_kvs(self.__drivers)
        ])

    async def cleanup(self) -> None:
        for driver in self.__drivers.values():
            try:
                await driver.cleanup()
            except Exception:
                get_logger().exception("Can't cleanup driver %s", driver)

    async def switch(self, channel: str, state: bool, wait: bool) -> None:
        gout = self.__outputs.get(channel)
        if gout is None:
            raise GpioChannelNotFoundError()
        await gout.switch(state, wait)

    async def pulse(self, channel: str, delay: float, wait: bool) -> None:
        gout = self.__outputs.get(channel)
        if gout is None:
            raise GpioChannelNotFoundError()
        await gout.pulse(delay, wait)

    # =====

    def __make_view(self) -> dict:
        return {
            "header": {"title": self.__make_view_title()},
            "table": self.__make_view_table(),
        }

    def __make_view_title(self) -> list[dict]:
        raw_title = self.__view["header"]["title"]
        title: list[dict] = []
        if isinstance(raw_title, list):
            for item in raw_title:
                if item.startswith("#") or len(item) == 0:
                    title.append(self.__make_item_label(item))
                else:
                    parts = list(map(str.strip, item.split("|", 2)))
                    if parts and parts[0] in self.__inputs:
                        title.append(self.__make_item_input(parts))
        else:
            title.append(self.__make_item_label(f"#{raw_title}"))
        return title

    def __make_view_table(self) -> list[list[dict] | None]:
        table: list[list[dict] | None] = []
        for row in self.__view["table"]:
            if len(row) == 0:
                table.append(None)
                continue

            items: list[dict] = []
            for item in map(str.strip, row):
                if item.startswith("#") or len(item) == 0:
                    items.append(self.__make_item_label(item))
                else:
                    parts = list(map(str.strip, item.split("|", 2)))
                    if parts:
                        if parts[0] in self.__inputs:
                            items.append(self.__make_item_input(parts))
                        elif parts[0] in self.__outputs:
                            items.append(self.__make_item_output(parts))
            table.append(items)
        return table

    def __make_item_label(self, item: str) -> dict:
        return {
            "type": "label",
            "text": item[1:].strip(),
        }

    def __make_item_input(self, parts: list[str]) -> dict:
        assert len(parts) >= 1
        color = (parts[1] if len(parts) > 1 else None)
        if color not in ["green", "yellow", "red"]:
            color = "green"
        return {
            "type": UserGpioModes.INPUT,
            "channel": parts[0],
            "color": color,
        }

    def __make_item_output(self, parts: list[str]) -> dict:
        assert len(parts) >= 1
        confirm = False
        text = "Click"
        if len(parts) == 2:
            text = parts[1]
        elif len(parts) == 3:
            confirm = (parts[1] == "confirm")
            text = parts[2]
        return {
            "type": UserGpioModes.OUTPUT,
            "channel": parts[0],
            "confirm": confirm,
            "text": text,
        }
