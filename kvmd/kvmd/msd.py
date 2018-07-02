import os
import asyncio
import types

from typing import Dict
from typing import NamedTuple
from typing import Callable
from typing import Type
from typing import Optional
from typing import Any

import pyudev

import aiofiles
import aiofiles.base

from .logging import get_logger


# =====
class MassStorageError(Exception):
    pass


class IsNotOperationalError(MassStorageError):
    def __init__(self) -> None:
        super().__init__("Missing bind for mass-storage device")


class AlreadyConnectedToPcError(MassStorageError):
    def __init__(self) -> None:
        super().__init__("Mass-storage is already connected to PC")


class AlreadyConnectedToKvmError(MassStorageError):
    def __init__(self) -> None:
        super().__init__("Mass-storage is already connected to KVM")


class IsNotConnectedToKvmError(MassStorageError):
    def __init__(self) -> None:
        super().__init__("Mass-storage is not connected to KVM")


class IsBusyError(MassStorageError):
    def __init__(self) -> None:
        super().__init__("Mass-storage is busy (write in progress)")


class DeviceInfo(NamedTuple):
    path: str
    bind: str
    size: int
    manufacturer: str
    product: str
    serial: str


def explore_device(path: str) -> DeviceInfo:
    # udevadm info -a -p  $(udevadm info -q path -n /dev/sda)
    ctx = pyudev.Context()

    block_device = pyudev.Devices.from_device_file(ctx, path)
    size = block_device.attributes.asint("size") * 512

    storage_device = block_device.find_parent("usb", "usb_interface")
    assert storage_device.driver == "usb-storage", (storage_device.driver, storage_device)

    usb_device = block_device.find_parent("usb", "usb_device")
    assert usb_device.driver == "usb", (usb_device.driver, usb_device)

    return DeviceInfo(
        path=path,
        bind=storage_device.sys_name,
        size=size,
        manufacturer=usb_device.attributes.asstring("manufacturer").strip(),
        product=usb_device.attributes.asstring("product").strip(),
        serial=usb_device.attributes.asstring("serial").strip(),
    )


def locate_by_bind(bind: str) -> str:
    ctx = pyudev.Context()
    for device in ctx.list_devices(subsystem="block"):
        storage_device = device.find_parent("usb", "usb_interface")
        if storage_device:
            try:
                device.attributes.asint("partititon")
            except KeyError:
                if storage_device.sys_name == bind:
                    return os.path.join("/dev", device.sys_name)
    return ""


def _operated_and_locked(method: Callable) -> Callable:
    async def wrap(self: "MassStorageDevice", *args: Any, **kwargs: Any) -> Any:
        if self._device_file:  # pylint: disable=protected-access
            raise IsBusyError()
        if not self._bind:  # pylint: disable=protected-access
            IsNotOperationalError()
        async with self._lock:  # pylint: disable=protected-access
            return (await method(self, *args, **kwargs))
    return wrap


class MassStorageDevice:
    def __init__(self, bind: str, init_delay: float, loop: asyncio.AbstractEventLoop) -> None:
        self._bind = bind
        self.__init_delay = init_delay
        self.__loop = loop

        self.__device_info: Optional[DeviceInfo] = None
        self._lock = asyncio.Lock()
        self._device_file: Optional[aiofiles.base.AiofilesContextManager] = None
        self.__writed = 0

        if self._bind:
            get_logger().info("Using bind %r as mass-storage device", self._bind)
            try:
                loop.run_until_complete(self.connect_to_kvm(no_delay=True))
            except Exception as err:
                if isinstance(err, MassStorageError):
                    log = get_logger().warning
                else:
                    log = get_logger().exception
                log("Mass-storage device is not operational: %s", err)
                self._bind = ""
        else:
            get_logger().warning("Missing bind; mass-storage device is not operational")

    @_operated_and_locked
    async def connect_to_kvm(self, no_delay: bool=False) -> None:
        if self.__device_info:
            raise AlreadyConnectedToKvmError()
        # TODO: disable gpio
        if not no_delay:
            await asyncio.sleep(self.__init_delay)
        path = locate_by_bind(self._bind)
        if not path:
            raise MassStorageError("Can't locate device by bind %r" % (self._bind))
        self.__device_info = explore_device(path)
        get_logger().info("Mass-storage device switched to KVM: %s", self.__device_info)

    @_operated_and_locked
    async def connect_to_pc(self) -> None:
        if not self.__device_info:
            raise AlreadyConnectedToPcError()
        # TODO: enable gpio
        self.__device_info = None
        get_logger().info("Mass-storage device switched to PC")

    def get_state(self) -> Dict:
        return {
            "in_operate": bool(self._bind),
            "connected_to": ("kvm" if self.__device_info else "pc"),
            "is_busy": bool(self._device_file),
            "writed": self.__writed,
            "info": (self.__device_info._asdict() if self.__device_info else None),
        }

    async def cleanup(self) -> None:
        async with self._lock:
            await self.__close_device_file()
            # TODO: disable gpio

    @_operated_and_locked
    async def __aenter__(self) -> "MassStorageDevice":
        if not self.__device_info:
            raise IsNotConnectedToKvmError()
        self._device_file = await aiofiles.open(self.__device_info.path, mode="wb", buffering=0)
        self.__writed = 0
        return self

    async def write(self, data: bytes) -> int:
        async with self._lock:
            assert self._device_file
            size = len(data)
            await self._device_file.write(data)
            await self._device_file.flush()
            await self.__loop.run_in_executor(None, os.fsync, self._device_file.fileno())
            self.__writed += size
            return self.__writed

    async def __aexit__(
        self,
        _exc_type: Type[BaseException],
        _exc: BaseException,
        _tb: types.TracebackType,
    ) -> None:
        async with self._lock:
            await self.__close_device_file()

    async def __close_device_file(self) -> None:
        try:
            if self._device_file:
                get_logger().info("Closing device file ...")
                await self._device_file.close()
        except Exception:
            get_logger().exception("Can't close device file")
            # TODO: reset device file
        self._device_file = None
        self.__writed = 0
