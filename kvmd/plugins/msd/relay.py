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


import os
import stat
import fcntl
import struct
import asyncio
import contextlib
import dataclasses

from typing import Dict
from typing import IO
from typing import AsyncGenerator
from typing import Optional

import aiofiles
import aiofiles.base
import gpiod

from ...logging import get_logger

from ... import env
from ... import aiotools
from ... import aiofs
from ... import aiogp

from ...yamlconf import Option

from ...validators.basic import valid_int_f1
from ...validators.basic import valid_float_f01
from ...validators.os import valid_abs_path
from ...validators.hw import valid_gpio_pin

from . import MsdError
from . import MsdIsBusyError
from . import MsdOfflineError
from . import MsdConnectedError
from . import MsdDisconnectedError
from . import MsdMultiNotSupported
from . import MsdCdromNotSupported
from . import BaseMsd


# =====
@dataclasses.dataclass(frozen=True)
class _ImageInfo:
    name: str
    size: int
    complete: bool


@dataclasses.dataclass(frozen=True)
class _DeviceInfo:
    path: str
    size: int
    free: int
    image: Optional[_ImageInfo]


_IMAGE_INFO_SIZE = 4096
_IMAGE_INFO_MAGIC_SIZE = 16
_IMAGE_INFO_NAME_SIZE = 256
_IMAGE_INFO_PADS_SIZE = _IMAGE_INFO_SIZE - _IMAGE_INFO_NAME_SIZE - 1 - 8 - _IMAGE_INFO_MAGIC_SIZE * 8
_IMAGE_INFO_FORMAT = ">%dL%dc?Q%dx%dL" % (
    _IMAGE_INFO_MAGIC_SIZE,
    _IMAGE_INFO_NAME_SIZE,
    _IMAGE_INFO_PADS_SIZE,
    _IMAGE_INFO_MAGIC_SIZE,
)
_IMAGE_INFO_MAGIC = [0x1ACE1ACE] * _IMAGE_INFO_MAGIC_SIZE


def _make_image_info_bytes(name: str, size: int, complete: bool) -> bytes:
    return struct.pack(
        _IMAGE_INFO_FORMAT,
        *_IMAGE_INFO_MAGIC,
        *memoryview((  # type: ignore
            name.encode("utf-8")
            + b"\x00" * _IMAGE_INFO_NAME_SIZE
        )[:_IMAGE_INFO_NAME_SIZE]).cast("c"),
        complete,
        size,
        *_IMAGE_INFO_MAGIC,
    )


def _parse_image_info_bytes(data: bytes) -> Optional[_ImageInfo]:
    try:
        parsed = list(struct.unpack(_IMAGE_INFO_FORMAT, data))
    except struct.error:
        pass
    else:
        magic_begin = parsed[:_IMAGE_INFO_MAGIC_SIZE]
        magic_end = parsed[-_IMAGE_INFO_MAGIC_SIZE:]
        if magic_begin == magic_end == _IMAGE_INFO_MAGIC:
            image_name_bytes = b"".join(parsed[
                _IMAGE_INFO_MAGIC_SIZE  # noqa: E203
                :
                _IMAGE_INFO_MAGIC_SIZE + _IMAGE_INFO_NAME_SIZE
            ])
            return _ImageInfo(
                name=image_name_bytes.decode("utf-8", errors="ignore").strip("\x00").strip(),
                size=parsed[_IMAGE_INFO_MAGIC_SIZE + _IMAGE_INFO_NAME_SIZE + 1],
                complete=parsed[_IMAGE_INFO_MAGIC_SIZE + _IMAGE_INFO_NAME_SIZE],
            )
    return None


def _ioctl_uint32(device_file: IO, request: int) -> int:
    buf = b"\0" * 4
    buf = fcntl.ioctl(device_file.fileno(), request, buf)  # type: ignore
    result = struct.unpack("I", buf)[0]
    assert result > 0, (device_file, request, buf)
    return result


def _explore_device(device_path: str) -> _DeviceInfo:
    if not stat.S_ISBLK(os.stat(device_path).st_mode):
        raise RuntimeError(f"Not a block device: {device_path}")

    with open(device_path, "rb") as device_file:
        # size = BLKGETSIZE * BLKSSZGET
        size = _ioctl_uint32(device_file, 0x1260) * _ioctl_uint32(device_file, 0x1268)
        device_file.seek(size - _IMAGE_INFO_SIZE)
        image_info = _parse_image_info_bytes(device_file.read())

    return _DeviceInfo(
        path=device_path,
        size=size,
        free=(size - image_info.size if image_info else size),
        image=image_info,
    )


class _Gpio:
    def __init__(
        self,
        target_pin: int,
        reset_pin: int,
        reset_delay: float,
    ) -> None:

        self.__target_pin = target_pin
        self.__reset_pin = reset_pin
        self.__reset_delay = reset_delay

        self.__chip: Optional[gpiod.Chip] = None
        self.__target_line: Optional[gpiod.Line] = None
        self.__reset_line: Optional[gpiod.Line] = None

    def open(self) -> None:
        assert self.__chip is None
        assert self.__target_line is None
        assert self.__reset_line is None

        self.__chip = gpiod.Chip(env.GPIO_DEVICE_PATH)

        self.__target_line = self.__chip.get_line(self.__target_pin)
        self.__target_line.request("kvmd::msd-relay::target", gpiod.LINE_REQ_DIR_OUT, default_vals=[0])

        self.__reset_line = self.__chip.get_line(self.__reset_pin)
        self.__reset_line.request("kvmd::msd-relay::reset", gpiod.LINE_REQ_DIR_OUT, default_vals=[0])

    def close(self) -> None:
        if self.__chip:
            try:
                self.__chip.close()
            except Exception:
                pass

    def switch_to_local(self) -> None:
        assert self.__target_line
        self.__target_line.set_value(0)

    def switch_to_server(self) -> None:
        assert self.__target_line
        self.__target_line.set_value(1)

    async def reset(self) -> None:
        assert self.__reset_line
        await aiogp.pulse(self.__reset_line, self.__reset_delay, 0)


# =====
class Plugin(BaseMsd):  # pylint: disable=too-many-instance-attributes
    def __init__(  # pylint: disable=super-init-not-called
        self,
        target_pin: int,
        reset_pin: int,

        device_path: str,
        init_delay: float,
        init_retries: int,
        reset_delay: float,
    ) -> None:

        self.__device_path = device_path
        self.__init_delay = init_delay
        self.__init_retries = init_retries

        self.__gpio = _Gpio(target_pin, reset_pin, reset_delay)

        self.__device_info: Optional[_DeviceInfo] = None
        self.__connected = False

        self.__device_file: Optional[aiofiles.base.AiofilesContextManager] = None
        self.__written = 0

        self.__notifier = aiotools.AioNotifier()
        self.__region = aiotools.AioExclusiveRegion(MsdIsBusyError, self.__notifier)

        logger = get_logger(0)
        logger.info("Using %r as MSD", self.__device_path)
        try:
            aiotools.run_sync(self.__load_device_info())
        except Exception as err:
            log = (logger.error if isinstance(err, MsdError) else logger.exception)
            log("MSD is offline: %s", err)

    @classmethod
    def get_plugin_options(cls) -> Dict:
        return {
            "target_pin": Option(-1, type=valid_gpio_pin),
            "reset_pin":  Option(-1, type=valid_gpio_pin),

            "device":       Option("",  type=valid_abs_path, unpack_as="device_path"),
            "init_delay":   Option(1.0, type=valid_float_f01),
            "init_retries": Option(5,   type=valid_int_f1),
            "reset_delay":  Option(1.0, type=valid_float_f01),
        }

    def sysprep(self) -> None:
        self.__gpio.open()

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
    async def connect(self) -> None:
        async with self.__working():
            async with self.__region:
                if self.__connected:
                    raise MsdConnectedError()

                self.__gpio.switch_to_server()
                self.__connected = True
                get_logger(0).info("MSD switched to Server")

    @aiotools.atomic
    async def disconnect(self) -> None:
        async with self.__working():
            async with self.__region:
                if not self.__connected:
                    raise MsdDisconnectedError()

                self.__gpio.switch_to_local()
                try:
                    await self.__load_device_info()
                except Exception:
                    if self.__connected:
                        self.__gpio.switch_to_server()
                    raise
                self.__connected = False
                get_logger(0).info("MSD switched to KVM: %s", self.__device_info)

    @contextlib.asynccontextmanager
    async def write_image(self, name: str) -> AsyncGenerator[None, None]:
        async with self.__working():
            async with self.__region:
                try:
                    assert self.__device_info
                    if self.__connected:
                        raise MsdConnectedError()

                    self.__device_file = await aiofiles.open(self.__device_info.path, mode="w+b", buffering=0)
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
        if self.__device_info.size - self.__written > _IMAGE_INFO_SIZE:
            await self.__device_file.seek(self.__device_info.size - _IMAGE_INFO_SIZE)
            await aiofs.afile_write_now(self.__device_file, _make_image_info_bytes(name, self.__written, complete))
            await self.__device_file.seek(0)
        else:
            get_logger().error("Can't write image info because device is full")

    async def __close_device_file(self) -> None:
        try:
            if self.__device_file:
                get_logger().info("Closing device file ...")
                await self.__device_file.close()
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
                self.__device_info = await aiotools.run_async(_explore_device, self.__device_path)
                break
            except Exception:
                if retries == 0:
                    self.__device_info = None
                    raise MsdError("Can't load device info")
                get_logger().exception("Can't load device info; retries=%d", retries)
                retries -= 1
