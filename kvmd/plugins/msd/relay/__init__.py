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
import contextlib
import dataclasses

from typing import Dict
from typing import AsyncGenerator
from typing import Optional

import aiofiles
import aiofiles.base

from ....logging import get_logger

from .... import aiotools
from .... import aiofs

from ....yamlconf import Option

from ....validators.basic import valid_bool
from ....validators.basic import valid_int_f1
from ....validators.basic import valid_float_f01
from ....validators.os import valid_abs_path
from ....validators.hw import valid_gpio_pin

from .. import MsdError
from .. import MsdIsBusyError
from .. import MsdOfflineError
from .. import MsdConnectedError
from .. import MsdDisconnectedError
from .. import MsdMultiNotSupported
from .. import MsdCdromNotSupported
from .. import BaseMsd

from .gpio import Gpio

from .drive import ImageInfo
from .drive import DeviceInfo


# =====
class Plugin(BaseMsd):  # pylint: disable=too-many-instance-attributes
    def __init__(  # pylint: disable=super-init-not-called
        self,
        gpio_device_path: str,
        target_pin: int,
        reset_inverted: bool,
        reset_pin: int,

        device_path: str,
        init_delay: float,
        init_retries: int,
        reset_delay: float,
    ) -> None:

        self.__device_path = device_path
        self.__init_delay = init_delay
        self.__init_retries = init_retries

        self.__gpio = Gpio(gpio_device_path, target_pin, reset_pin, reset_inverted, reset_delay)

        self.__device_info: Optional[DeviceInfo] = None
        self.__connected = False

        self.__device_file: Optional[aiofiles.base.AiofilesContextManager] = None
        self.__written = 0

        self.__notifier = aiotools.AioNotifier()
        self.__region = aiotools.AioExclusiveRegion(MsdIsBusyError, self.__notifier)

    @classmethod
    def get_plugin_options(cls) -> Dict:
        return {
            "gpio_device":    Option("/dev/gpiochip0", type=valid_abs_path, unpack_as="gpio_device_path"),
            "target_pin":     Option(-1, type=valid_gpio_pin),
            "reset_pin":      Option(-1, type=valid_gpio_pin),
            "reset_inverted": Option(False, type=valid_bool),

            "device":       Option("",  type=valid_abs_path, unpack_as="device_path"),
            "init_delay":   Option(1.0, type=valid_float_f01),
            "init_retries": Option(5,   type=valid_int_f1),
            "reset_delay":  Option(1.0, type=valid_float_f01),
        }

    def sysprep(self) -> None:
        logger = get_logger(0)
        self.__gpio.open()
        logger.info("Using %r as MSD", self.__device_path)
        try:
            aiotools.run_sync(self.__load_device_info())
        except Exception as err:
            log = (logger.error if isinstance(err, MsdError) else logger.exception)
            log("MSD is offline: %s", err)

    async def get_state(self) -> Dict:
        storage: Optional[Dict] = None
        drive: Optional[Dict] = None
        if self.__device_info:
            storage = {
                "size": self.__device_info.size,
                "free": self.__device_info.free,
                "uploading": bool(self.__device_file)
            }
            drive = {
                "image": (self.__device_info.image and dataclasses.asdict(self.__device_info.image)),
                "connected": self.__connected,
            }
        return {
            "enabled": True,
            "online": bool(self.__device_info),
            "busy": self.__region.is_busy(),
            "storage": storage,
            "drive": drive,
            "features": {
                "multi": False,
                "cdrom": False,
            },
        }

    async def poll_state(self) -> AsyncGenerator[Dict, None]:
        prev_state: Dict = {}
        while True:
            state = await self.get_state()
            if state != prev_state:
                yield state
                prev_state = state
            await self.__notifier.wait()

    @aiotools.atomic
    async def reset(self) -> None:
        await aiotools.run_region_task(
            "Can't reset MSD or operation was not completed",
            self.__region, self.__inner_reset,
        )

    @aiotools.atomic
    async def __inner_reset(self) -> None:
        await self.__gpio.reset()
        self.__gpio.switch_to_local()
        self.__connected = False
        await self.__load_device_info()
        get_logger(0).info("MSD reset has been successful")

    @aiotools.atomic
    async def cleanup(self) -> None:
        try:
            await self.__close_device_file()
        finally:
            self.__gpio.close()

    # =====

    @aiotools.atomic
    async def set_params(self, name: Optional[str]=None, cdrom: Optional[bool]=None) -> None:
        async with self.__working():
            if name is not None:
                raise MsdMultiNotSupported()
            if cdrom is not None:
                raise MsdCdromNotSupported()

    @aiotools.atomic
    async def set_connected(self, connected: bool) -> None:
        async with self.__working():
            async with self.__region:
                if connected:
                    if self.__connected:
                        raise MsdConnectedError()
                    self.__gpio.switch_to_server()
                    get_logger(0).info("MSD switched to Server")
                else:
                    if not self.__connected:
                        raise MsdDisconnectedError()
                    self.__gpio.switch_to_local()
                    try:
                        await self.__load_device_info()
                    except Exception:
                        if self.__connected:
                            self.__gpio.switch_to_server()
                        raise
                    get_logger(0).info("MSD switched to KVM: %s", self.__device_info)
                self.__connected = connected

    @contextlib.asynccontextmanager
    async def write_image(self, name: str) -> AsyncGenerator[None, None]:
        async with self.__working():
            async with self.__region:
                try:
                    assert self.__device_info
                    if self.__connected:
                        raise MsdConnectedError()

                    self.__device_file = await aiofiles.open(self.__device_info.path, mode="w+b", buffering=0)  # type: ignore
                    self.__written = 0

                    await self.__write_image_info(name, complete=False)
                    await self.__notifier.notify()
                    yield
                    await self.__write_image_info(name, complete=True)
                finally:
                    await self.__close_device_file()
                    await self.__load_device_info()

    async def write_image_chunk(self, chunk: bytes) -> int:
        assert self.__device_file
        await aiofs.afile_write_now(self.__device_file, chunk)
        self.__written += len(chunk)
        return self.__written

    @aiotools.atomic
    async def remove(self, name: str) -> None:
        async with self.__working():
            raise MsdMultiNotSupported()

    # =====

    @contextlib.asynccontextmanager
    async def __working(self) -> AsyncGenerator[None, None]:
        if not self.__device_info:
            raise MsdOfflineError()
        yield

    # =====

    async def __write_image_info(self, name: str, complete: bool) -> None:
        assert self.__device_file
        assert self.__device_info
        if not (await self.__device_info.write_image_info(
            device_file=self.__device_file,
            image_info=ImageInfo(name, self.__written, complete),
        )):
            get_logger().error("Can't write image info because device is full")

    async def __close_device_file(self) -> None:
        try:
            if self.__device_file:
                get_logger().info("Closing device file ...")
                await self.__device_file.close()  # type: ignore
        except Exception:
            get_logger().exception("Can't close device file")
        finally:
            self.__device_file = None
            self.__written = 0

    async def __load_device_info(self) -> None:
        retries = self.__init_retries
        while True:
            await asyncio.sleep(self.__init_delay)
            try:
                self.__device_info = await DeviceInfo.read(self.__device_path)
                break
            except Exception:
                if retries == 0:
                    self.__device_info = None
                    raise MsdError("Can't load device info")
                get_logger().exception("Can't load device info; retries=%d", retries)
                retries -= 1
