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


import contextlib
import functools

from typing import Callable
from typing import Any

import hid

from ...logging import get_logger

from ... import tools
from ... import aiotools

from ...yamlconf import Option

from ...validators.basic import valid_number
from ...validators.os import valid_abs_path

from . import GpioDriverOfflineError
from . import UserGpioModes
from . import BaseUserGpioDriver


# =====
class Plugin(BaseUserGpioDriver):
    # This is like a HID relay, but does not support the common protocol.
    # So no status reports, ugh.
    # Why make a HID USB if you can't implement such simple things?
    # So many questions, and so few answers...

    def __init__(  # pylint: disable=super-init-not-called
        self,
        instance_name: str,
        notifier: aiotools.AioNotifier,

        device_path: str,
    ) -> None:

        super().__init__(instance_name, notifier)

        self.__device_path = device_path

        self.__device: (hid.device | None) = None  # type: ignore
        self.__stop = False

        self.__initials: dict[int, bool] = {}
        self.__state: dict[int, bool] = dict.fromkeys(range(8), False)

    @classmethod
    def get_plugin_options(cls) -> dict:
        return {
            "device": Option("",  type=valid_abs_path, unpack_as="device_path"),
        }

    @classmethod
    def get_modes(cls) -> set[str]:
        return set([UserGpioModes.OUTPUT])

    @classmethod
    def get_pin_validator(cls) -> Callable[[Any], Any]:
        return functools.partial(valid_number, min=0, max=7, name="NOYITO relay channel")

    def register_output(self, pin: str, initial: (bool | None)) -> None:
        self.__initials[int(pin)] = bool(initial)

    def prepare(self) -> None:
        logger = get_logger(0)
        logger.info("Probing driver %s on %s ...", self, self.__device_path)
        try:
            with self.__ensure_device("probing"):
                pass
        except Exception as err:
            logger.error("Can't probe %s on %s: %s",
                         self, self.__device_path, tools.efmt(err))
        self.__reset_pins()

    async def cleanup(self) -> None:
        self.__reset_pins()
        self.__close_device()
        self.__stop = True

    async def read(self, pin: str) -> bool:
        return self.__state[int(pin)]

    async def write(self, pin: str, state: bool) -> None:
        try:
            return self.__inner_write(int(pin), state)
        except Exception:
            raise GpioDriverOfflineError(self)

    # =====

    def __reset_pins(self) -> None:
        logger = get_logger(0)
        for (pin, state) in self.__initials.items():
            logger.info("Resetting pin=%d to state=%d of %s on %s: ...",
                        pin, state, self, self.__device_path)
            try:
                self.__inner_write(pin, state)
            except Exception as err:
                logger.error("Can't reset pin=%d of %s on %s: %s",
                             pin, self, self.__device_path, tools.efmt(err))

    def __inner_write(self, pin: int, state: bool) -> None:
        assert 0 <= pin <= 7
        with self.__ensure_device("writing") as device:
            report = [0xA0, pin + 1, int(state), 0]
            report[-1] = sum(report)
            result = device.write(report)
            if result < 0:
                raise RuntimeError(f"Retval of send_feature_report() < 0: {result}")
            self.__state[pin] = state

    @contextlib.contextmanager
    def __ensure_device(self, context: str) -> hid.device:  # type: ignore
        assert not self.__stop
        if self.__device is None:
            device = hid.device()  # type: ignore
            device.open_path(self.__device_path.encode("utf-8"))
            device.set_nonblocking(True)
            self.__device = device
            get_logger(0).info("Opened %s on %s while %s", self, self.__device_path, context)
        try:
            yield self.__device
        except Exception as err:
            get_logger(0).error("Error occured on %s on %s while %s: %s",
                                self, self.__device_path, context, tools.efmt(err))
            self.__close_device()
            raise

    def __close_device(self) -> None:
        if self.__device:
            try:
                self.__device.close()
            except Exception:
                pass
            self.__device = None
            get_logger(0).info("Closed %s on %s", self, self.__device_path)

    def __str__(self) -> str:
        return f"Noyito({self._instance_name})"

    __repr__ = __str__
