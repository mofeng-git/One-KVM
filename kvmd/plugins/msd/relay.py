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
import asyncio.queues
import dataclasses
import types

from typing import Dict
from typing import IO
from typing import Callable
from typing import Type
from typing import AsyncGenerator
from typing import Optional
from typing import Any

import aiofiles
import aiofiles.base

from ...logging import get_logger

from ... import aiotools
from ... import aioregion
from ... import gpio

from ...yamlconf import Option

from ...validators.basic import valid_bool
from ...validators.basic import valid_number
from ...validators.basic import valid_int_f1
from ...validators.basic import valid_float_f01

from ...validators.os import valid_abs_path

from ...validators.hw import valid_gpio_pin

from . import MsdError
from . import MsdOfflineError
from . import MsdAlreadyOnServerError
from . import MsdAlreadyOnKvmError
from . import MsdNotOnKvmError
from . import MsdIsBusyError
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
    real: str
    size: int
    image: Optional[_ImageInfo]


_IMAGE_INFO_SIZE = 4096
_IMAGE_INFO_MAGIC_SIZE = 16
_IMAGE_INFO_IMAGE_NAME_SIZE = 256
_IMAGE_INFO_PADS_SIZE = _IMAGE_INFO_SIZE - _IMAGE_INFO_IMAGE_NAME_SIZE - 1 - 8 - _IMAGE_INFO_MAGIC_SIZE * 8
_IMAGE_INFO_FORMAT = ">%dL%dc?Q%dx%dL" % (
    _IMAGE_INFO_MAGIC_SIZE,
    _IMAGE_INFO_IMAGE_NAME_SIZE,
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
            + b"\x00" * _IMAGE_INFO_IMAGE_NAME_SIZE
        )[:_IMAGE_INFO_IMAGE_NAME_SIZE]).cast("c"),
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
            image_name_bytes = b"".join(parsed[_IMAGE_INFO_MAGIC_SIZE:_IMAGE_INFO_MAGIC_SIZE + _IMAGE_INFO_IMAGE_NAME_SIZE])
            return _ImageInfo(
                name=image_name_bytes.decode("utf-8", errors="ignore").strip("\x00").strip(),
                size=parsed[_IMAGE_INFO_MAGIC_SIZE + _IMAGE_INFO_IMAGE_NAME_SIZE + 1],
                complete=parsed[_IMAGE_INFO_MAGIC_SIZE + _IMAGE_INFO_IMAGE_NAME_SIZE],
            )
    return None


def _ioctl_uint32(device_file: IO, request: int) -> int:
    buf = b"\0" * 4
    buf = fcntl.ioctl(device_file.fileno(), request, buf)
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
        real=os.path.realpath(device_path),
        size=size,
        image=image_info,
    )


def _msd_working(method: Callable) -> Callable:
    async def wrapper(self: "Plugin", *args: Any, **kwargs: Any) -> Any:
        if not self._device_info:  # pylint: disable=protected-access
            raise MsdOfflineError()
        return (await method(self, *args, **kwargs))
    return wrapper


class Plugin(BaseMsd):  # pylint: disable=too-many-instance-attributes
    def __init__(  # pylint: disable=super-init-not-called
        self,
        target_pin: int,
        reset_pin: int,

        device_path: str,
        init_delay: float,
        init_retries: int,
        reset_delay: float,
        write_meta: bool,
        chunk_size: int,
    ) -> None:

        self.__target_pin = gpio.set_output(target_pin)
        self.__reset_pin = gpio.set_output(reset_pin)

        self.__device_path = device_path
        self.__init_delay = init_delay
        self.__init_retries = init_retries
        self.__reset_delay = reset_delay
        self.__write_meta = write_meta
        self.__chunk_size = chunk_size

        self.__region = aioregion.AioExclusiveRegion(MsdIsBusyError)

        self._device_info: Optional[_DeviceInfo] = None
        self.__device_file: Optional[aiofiles.base.AiofilesContextManager] = None
        self.__written = 0
        self.__on_kvm = True

        self.__state_queue: asyncio.queues.Queue = asyncio.Queue()

        logger = get_logger(0)
        logger.info("Using %r as MSD", self.__device_path)
        try:
            aiotools.run_sync(self.__load_device_info())
            if self.__write_meta:
                logger.info("Enabled image metadata writing")
        except Exception as err:
            log = (logger.error if isinstance(err, MsdError) else logger.exception)
            log("MSD is offline: %s", err)

    @classmethod
    def get_plugin_options(cls) -> Dict[str, Option]:
        return {
            "target_pin": Option(-1, type=valid_gpio_pin),
            "reset_pin":  Option(-1, type=valid_gpio_pin),

            "device":       Option("",    type=valid_abs_path, unpack_as="device_path"),
            "init_delay":   Option(1.0,   type=valid_float_f01),
            "init_retries": Option(5,     type=valid_int_f1),
            "reset_delay":  Option(1.0,   type=valid_float_f01),
            "write_meta":   Option(True,  type=valid_bool),
            "chunk_size":   Option(65536, type=(lambda arg: valid_number(arg, min=1024))),
        }

    def get_state(self) -> Dict:
        online = bool(self._device_info)
        return {
            "enabled": True,
            "online": online,
            "busy": self.__region.is_busy(),
            "uploading": bool(self.__device_file),
            "written": self.__written,
            "info": (dataclasses.asdict(self._device_info) if online else None),
            "connected_to": (("kvm" if self.__on_kvm else "server") if online else None),
        }

    async def poll_state(self) -> AsyncGenerator[Dict, None]:
        while True:
            yield (await self.__state_queue.get())

    @aiotools.atomic
    async def cleanup(self) -> None:
        await self.__close_device_file()
        gpio.write(self.__target_pin, False)
        gpio.write(self.__reset_pin, False)

    @_msd_working
    @aiotools.atomic
    async def connect_to_kvm(self) -> Dict:
        notify = False
        state: Dict = {}
        try:
            with self.__region:
                if self.__on_kvm:
                    raise MsdAlreadyOnKvmError()
                notify = True

                gpio.write(self.__target_pin, False)
                try:
                    await self.__load_device_info()
                except Exception:
                    if not self.__on_kvm:
                        gpio.write(self.__target_pin, True)
                    raise
                self.__on_kvm = True
                get_logger().info("MSD switched to KVM: %s", self._device_info)

            state = self.get_state()
            return state
        finally:
            if notify:
                await self.__state_queue.put(state or self.get_state())

    @_msd_working
    @aiotools.atomic
    async def connect_to_server(self) -> Dict:
        notify = False
        state: Dict = {}
        try:
            with self.__region:
                if not self.__on_kvm:
                    raise MsdAlreadyOnServerError()
                notify = True

                gpio.write(self.__target_pin, True)
                self.__on_kvm = False
                get_logger().info("MSD switched to Server")

            state = self.get_state()
            return state
        finally:
            if notify:
                await self.__state_queue.put(state or self.get_state())

    @aiotools.atomic
    async def reset(self) -> None:
        with aiotools.unregion_only_on_exception(self.__region):
            await self.__inner_reset()

    @aiotools.tasked
    @aiotools.muted("Can't reset MSD or operation was not completed")
    async def __inner_reset(self) -> None:
        try:
            gpio.write(self.__reset_pin, True)
            await asyncio.sleep(self.__reset_delay)
            gpio.write(self.__reset_pin, False)

            gpio.write(self.__target_pin, False)
            self.__on_kvm = True

            await self.__load_device_info()
            get_logger(0).info("MSD reset has been successful")
        finally:
            try:
                gpio.write(self.__reset_pin, False)
            finally:
                try:
                    await self.__state_queue.put(self.get_state())
                finally:
                    self.__region.exit()

    @_msd_working
    @aiotools.atomic
    async def __aenter__(self) -> "Plugin":
        assert self._device_info
        self.__region.enter()
        try:
            if not self.__on_kvm:
                raise MsdNotOnKvmError()
            self.__device_file = await aiofiles.open(self._device_info.path, mode="w+b", buffering=0)
            self.__written = 0
            return self
        except Exception:
            self.__region.exit()
            raise
        finally:
            await self.__state_queue.put(self.get_state())

    def get_chunk_size(self) -> int:
        return self.__chunk_size

    @aiotools.atomic
    async def write_image_info(self, name: str, complete: bool) -> None:
        assert self.__device_file
        assert self._device_info
        if self.__write_meta:
            if self._device_info.size - self.__written > _IMAGE_INFO_SIZE:
                await self.__device_file.seek(self._device_info.size - _IMAGE_INFO_SIZE)
                await self.__write_to_device_file(_make_image_info_bytes(name, self.__written, complete))
                await self.__device_file.seek(0)
            else:
                get_logger().error("Can't write image info because device is full")

    @aiotools.atomic
    async def write_image_chunk(self, chunk: bytes) -> int:
        await self.__write_to_device_file(chunk)
        self.__written += len(chunk)
        return self.__written

    @aiotools.atomic
    async def __aexit__(
        self,
        _exc_type: Type[BaseException],
        _exc: BaseException,
        _tb: types.TracebackType,
    ) -> None:

        try:
            await self.__close_device_file()
            await self.__load_device_info()
        finally:
            self.__region.exit()
            await self.__state_queue.put(self.get_state())

    async def __write_to_device_file(self, data: bytes) -> None:
        assert self.__device_file
        await self.__device_file.write(data)
        await self.__device_file.flush()
        await aiotools.run_async(os.fsync, self.__device_file.fileno())

    async def __close_device_file(self) -> None:
        try:
            if self.__device_file:
                get_logger().info("Closing device file ...")
                await self.__device_file.close()
        except asyncio.CancelledError:  # pylint: disable=try-except-raise
            raise
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
                self._device_info = await aiotools.run_async(_explore_device, self.__device_path)
                break
            except asyncio.CancelledError:  # pylint: disable=try-except-raise
                raise
            except Exception:
                if retries == 0:
                    self._device_info = None
                    raise MsdError("Can't load device info")
                get_logger().exception("Can't load device info; retries=%d", retries)
                retries -= 1
