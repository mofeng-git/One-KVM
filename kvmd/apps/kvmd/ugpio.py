# ========================================================================== #
#                                                                            #
#    KVMD - The main Pi-KVM daemon.                                          #
#                                                                            #
#    Copyright (C) 2018  Maxim Devaev <mdevaev@gmail.com>                    #
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
import operator

from typing import List
from typing import Dict
from typing import AsyncGenerator
from typing import Optional

from ...logging import get_logger

from ... import aiotools
from ... import gpio

from ...yamlconf import Section

from ...errors import OperationError
from ...errors import IsBusyError


# =====
class GpioChannelNotFoundError(OperationError):
    def __init__(self) -> None:
        super().__init__("GPIO channel is not found")


class GpioSwitchNotSupported(OperationError):
    def __init__(self) -> None:
        super().__init__("This GPIO channel does not support switching")


class GpioPulseNotSupported(OperationError):
    def __init__(self) -> None:
        super().__init__("This GPIO channel does not support pulsing")


class GpioChannelIsBusyError(IsBusyError):
    def __init__(self) -> None:
        super().__init__("Performing another GPIO operation on this channel, please try again later")


# =====
class _GpioInput:
    def __init__(self, channel: str, config: Section, reader: gpio.BatchReader) -> None:
        self.__channel = channel
        self.__pin: int = config.pin
        self.__inverted: bool = config.inverted

        self.__reader = reader

    def get_scheme(self) -> Dict:
        return {}

    def get_state(self) -> Dict:
        return {"state": (self.__reader.get(self.__pin) ^ self.__inverted)}

    def __str__(self) -> str:
        return f"Input({self.__channel}, pin={self.__pin})"

    __repr__ = __str__


class _GpioOutput:  # pylint: disable=too-many-instance-attributes
    def __init__(self, channel: str, config: Section, notifier: aiotools.AioNotifier) -> None:
        self.__channel = channel
        self.__pin: int = config.pin
        self.__inverted: bool = config.inverted

        self.__switch: bool = config.switch

        self.__pulse_delay: float = config.pulse.delay
        self.__min_pulse_delay: float = config.pulse.min_delay
        self.__max_pulse_delay: float = config.pulse.max_delay

        self.__busy_delay: float = config.busy_delay

        self.__region = aiotools.AioExclusiveRegion(GpioChannelIsBusyError, notifier)

    def get_scheme(self) -> Dict:
        return {
            "switch": self.__switch,
            "pulse": {
                "delay": self.__pulse_delay,
                "min_delay": (self.__min_pulse_delay if self.__pulse_delay else 0),
                "max_delay": (self.__max_pulse_delay if self.__pulse_delay else 0),
            },
        }

    def get_state(self) -> Dict:
        busy = self.__region.is_busy()
        return {
            "state": (self.__read() if not busy else False),
            "busy": busy,
        }

    def cleanup(self) -> None:
        try:
            gpio.write(self.__pin, False)
        except Exception:
            get_logger().exception("Can't cleanup GPIO %s", self)

    async def switch(self, state: bool) -> bool:
        if not self.__switch:
            raise GpioSwitchNotSupported()
        async with self.__region:
            if state != self.__read():
                self.__write(state)
                get_logger(0).info("Switched %s to %d", self, state)
                await asyncio.sleep(self.__busy_delay)
                return True
            await asyncio.sleep(self.__busy_delay)
            return False

    @aiotools.atomic
    async def pulse(self, delay: float) -> None:
        if not self.__pulse_delay:
            raise GpioPulseNotSupported()
        delay = min(max((delay or self.__pulse_delay), self.__min_pulse_delay), self.__max_pulse_delay)
        await aiotools.run_region_task(
            f"Can't perform pulse of {self} or operation was not completed",
            self.__region, self.__inner_pulse, delay,
        )

    @aiotools.atomic
    async def __inner_pulse(self, delay: float) -> None:
        try:
            self.__write(True)
            await asyncio.sleep(delay)
        finally:
            self.__write(False)
            await asyncio.sleep(self.__busy_delay)
        get_logger(0).info("Pulsed %s with delay=%.2f", self, delay)

    def __read(self) -> bool:
        return (gpio.read(self.__pin) ^ self.__inverted)

    def __write(self, state: bool) -> None:
        gpio.write(self.__pin, (state ^ self.__inverted))

    def __str__(self) -> str:
        return f"Output({self.__channel}, pin={self.__pin})"

    __repr__ = __str__


# =====
class UserGpio:
    def __init__(self, config: Section) -> None:
        self.__view = config.view

        self.__state_notifier = aiotools.AioNotifier()
        self.__reader = gpio.BatchReader(
            pins=[
                (
                    gpio.set_input(ch_config.pin)
                    if ch_config.mode == "input" else
                    gpio.set_output(ch_config.pin, (ch_config.initial ^ ch_config.inverted))
                )
                for ch_config in config.scheme.values()
            ],
            interval=config.state_poll,
            notifier=self.__state_notifier,
        )

        self.__inputs: Dict[str, _GpioInput] = {}
        self.__outputs: Dict[str, _GpioOutput] = {}

        for (channel, ch_config) in sorted(config.scheme.items(), key=operator.itemgetter(0)):
            if ch_config.mode == "input":
                self.__inputs[channel] = _GpioInput(channel, ch_config, self.__reader)
            else:  # output:
                self.__outputs[channel] = _GpioOutput(channel, ch_config, self.__state_notifier)

    async def get_model(self) -> Dict:
        return {
            "scheme": {
                "inputs": {channel: gin.get_scheme() for (channel, gin) in self.__inputs.items()},
                "outputs": {channel: gout.get_scheme() for (channel, gout) in self.__outputs.items()},
            },
            "view": self.__make_view(),
        }

    async def get_state(self) -> Dict:
        return {
            "inputs": {channel: gin.get_state() for (channel, gin) in self.__inputs.items()},
            "outputs": {channel: gout.get_state() for (channel, gout) in self.__outputs.items()},
        }

    async def poll_state(self) -> AsyncGenerator[Dict, None]:
        prev_state: Dict = {}
        while True:
            state = await self.get_state()
            if state != prev_state:
                yield state
                prev_state = state
            await self.__state_notifier.wait()

    async def systask(self) -> None:
        await self.__reader.poll()

    async def cleanup(self) -> None:
        for gout in self.__outputs.values():
            gout.cleanup()

    async def switch(self, channel: str, state: bool) -> bool:
        gout = self.__outputs.get(channel)
        if gout is None:
            raise GpioChannelNotFoundError()
        return (await gout.switch(state))

    async def pulse(self, channel: str, delay: float) -> None:
        gout = self.__outputs.get(channel)
        if gout is None:
            raise GpioChannelNotFoundError()
        await gout.pulse(delay)

    # =====

    def __make_view(self) -> Dict:
        table: List[Optional[List[Dict]]] = []
        for row in self.__view["table"]:
            if len(row) == 0:
                table.append(None)
                continue

            items: List[Dict] = []
            for item in map(str.strip, row):
                if item.startswith("#") or len(item) == 0:
                    items.append({
                        "type": "label",
                        "text": item[1:].strip(),
                    })
                else:
                    parts = list(map(str.strip, item.split(",", 1)))
                    if parts:
                        if parts[0] in self.__inputs:
                            items.append({
                                "type": "input",
                                "channel": parts[0],
                            })
                        elif parts[0] in self.__outputs:
                            items.append({
                                "type": "output",
                                "channel": parts[0],
                                "text": (parts[1] if len(parts) > 1 else "Click"),
                            })
            table.append(items)
        return {
            "header": self.__view["header"],
            "table": table,
        }
