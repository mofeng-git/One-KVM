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
import asyncio
import contextlib
import dataclasses

from typing import List
from typing import Dict
from typing import AsyncGenerator
from typing import Optional

import aiofiles
import aiofiles.base

from ....logging import get_logger

from ....inotify import InotifyMask
from ....inotify import Inotify

from ....yamlconf import Option

from ....validators.os import valid_abs_dir
from ....validators.os import valid_command

from .... import aiotools
from .... import aioregion

from .. import MsdError
from .. import MsdOfflineError
from .. import MsdConnectedError
from .. import MsdDisconnectedError
from .. import MsdImageNotSelected
from .. import MsdUnknownImageError
from .. import MsdImageExistsError
from .. import MsdIsBusyError
from .. import BaseMsd

from .drive import Drive

from .helpers import remount_storage
from .helpers import unlock_drive


# =====
@dataclasses.dataclass(frozen=True)
class _DriveImage:
    name: str
    path: str
    size: int
    complete: bool
    in_storage: bool


@dataclasses.dataclass(frozen=True)
class _DriveState:
    image: Optional[_DriveImage]
    cdrom: bool
    rw: bool


@dataclasses.dataclass(frozen=True)
class _StorageState:
    size: int
    free: int
    images: Dict[str, _DriveImage]


# =====
@dataclasses.dataclass
class _VirtualDriveState:
    image: Optional[_DriveImage]
    connected: bool
    cdrom: bool

    @classmethod
    def from_drive_state(cls, state: _DriveState) -> "_VirtualDriveState":
        return _VirtualDriveState(
            image=state.image,
            connected=bool(state.image),
            cdrom=state.cdrom,
        )


class _State:
    def __init__(self, changes_queue: asyncio.queues.Queue) -> None:
        self.__changes_queue = changes_queue

        self.storage: Optional[_StorageState] = None
        self.vd: Optional[_VirtualDriveState] = None

        self._lock = asyncio.Lock()
        self._region = aioregion.AioExclusiveRegion(MsdIsBusyError)

    @contextlib.asynccontextmanager
    async def busy(self, check_online: bool=True) -> AsyncGenerator[None, None]:
        with self._region:
            async with self._lock:
                await self.__changes_queue.put(None)
                if check_online:
                    if self.vd is None:
                        raise MsdOfflineError()
                    assert self.storage
                yield
        await self.__changes_queue.put(None)

    def is_busy(self) -> bool:
        return self._region.is_busy()


# =====
class Plugin(BaseMsd):  # pylint: disable=too-many-instance-attributes
    def __init__(  # pylint: disable=super-init-not-called
        self,
        storage_path: str,

        remount_cmd: List[str],
        unlock_cmd: List[str],

        sysfs_prefix: str,
        gadget: str,  # XXX: Not from options, see /kvmd/apps/kvmd/__init__.py for details
    ) -> None:

        self.__storage_path = os.path.normpath(storage_path)
        self.__images_path = os.path.join(self.__storage_path, "images")
        self.__meta_path = os.path.join(self.__storage_path, "meta")

        self.__remount_cmd = remount_cmd
        self.__unlock_cmd = unlock_cmd

        self.__drive = Drive(sysfs_prefix, gadget, instance=0, lun=0)

        self.__new_file: Optional[aiofiles.base.AiofilesContextManager] = None
        self.__new_file_written = 0

        self.__changes_queue: asyncio.queues.Queue = asyncio.Queue()

        self.__state = _State(self.__changes_queue)

        logger = get_logger(0)
        logger.info("Using OTG gadget %r as MSD", gadget)
        aiotools.run_sync(self.__reload_state())

    @classmethod
    def get_plugin_options(cls) -> Dict:
        sudo = ["/usr/bin/sudo", "--non-interactive"]
        return {
            "storage":      Option("/var/lib/kvmd/msd", type=valid_abs_dir, unpack_as="storage_path"),
            "remount_cmd":  Option([*sudo, "/usr/bin/kvmd-helper-otgmsd-remount", "{mode}"], type=valid_command),
            "unlock_cmd":   Option([*sudo, "/usr/bin/kvmd-helper-otgmsd-unlock", "unlock"],  type=valid_command),
            "sysfs_prefix": Option("", type=(lambda arg: str(arg).strip())),
        }

    async def get_state(self) -> Dict:
        async with self.__state._lock:  # pylint: disable=protected-access
            storage: Optional[Dict] = None
            if self.__state.storage:
                storage = dataclasses.asdict(self.__state.storage)
                for name in list(storage["images"]):
                    del storage["images"][name]["path"]
                    del storage["images"][name]["in_storage"]
                storage["uploading"] = bool(self.__new_file)

            vd: Optional[Dict] = None
            if self.__state.vd:
                vd = dataclasses.asdict(self.__state.vd)
                if vd["image"]:
                    del vd["image"]["path"]

            return {
                "enabled": False,  # FIXME
                "online": bool(self.__state.vd),
                "busy": self.__state.is_busy(),
                "storage": storage,
                "drive": vd,
                "features": {
                    "multi": True,
                    "cdrom": True,
                },
            }

    async def poll_state(self) -> AsyncGenerator[Dict, None]:
        inotify_task = asyncio.create_task(self.__watch_inotify())
        prev_state: Dict = {}
        try:
            while True:
                if inotify_task.cancelled():
                    break
                if inotify_task.done():
                    RuntimeError("Inotify task is dead")

                try:
                    await asyncio.wait_for(self.__changes_queue.get(), timeout=0.1)
                except asyncio.TimeoutError:
                    continue

                state = await self.get_state()
                if state != prev_state:
                    yield state
                    prev_state = state
        finally:
            if not inotify_task.done():
                inotify_task.cancel()
                await inotify_task

    @aiotools.atomic
    async def reset(self) -> None:
        async with self.__state.busy(check_online=False):
            try:
                await self.__unlock_drive()
                self.__drive.set_image_path("")
                self.__drive.set_rw_flag(False)
                self.__drive.set_cdrom_flag(False)
            except asyncio.CancelledError:
                raise
            except Exception:
                get_logger(0).exception("Can't reset MSD")

    @aiotools.atomic
    async def cleanup(self) -> None:
        await self.__close_new_file()

    # =====

    @aiotools.atomic
    async def set_params(self, name: Optional[str]=None, cdrom: Optional[bool]=None) -> None:
        async with self.__state.busy():
            assert self.__state.storage
            assert self.__state.vd

            if self.__state.vd.connected or self.__drive.get_image_path():
                raise MsdConnectedError()

            if name is not None:
                if name:
                    image = self.__state.storage.images.get(name)
                    if image is None or not os.path.exists(image.path):
                        raise MsdUnknownImageError()
                    assert image.in_storage
                    self.__state.vd.image = image
                else:
                    self.__state.vd.image = None

            if cdrom is not None:
                self.__state.vd.cdrom = cdrom

    @aiotools.atomic
    async def connect(self) -> None:
        async with self.__state.busy():
            assert self.__state.vd

            if self.__state.vd.connected or self.__drive.get_image_path():
                raise MsdConnectedError()
            if self.__state.vd.image is None:
                raise MsdImageNotSelected()

            assert self.__state.vd.image.in_storage

            if not os.path.exists(self.__state.vd.image.path):
                raise MsdUnknownImageError()

            await self.__unlock_drive()
            self.__drive.set_cdrom_flag(self.__state.vd.cdrom)
            self.__drive.set_image_path(self.__state.vd.image.path)
            self.__state.vd.connected = True

    @aiotools.atomic
    async def disconnect(self) -> None:
        async with self.__state.busy():
            assert self.__state.vd

            if not (self.__state.vd.connected or self.__drive.get_image_path()):
                raise MsdDisconnectedError()

            await self.__unlock_drive()
            self.__drive.set_image_path("")
            self.__state.vd.connected = False

    @contextlib.asynccontextmanager
    async def write_image(self, name: str) -> AsyncGenerator[None, None]:
        try:
            with self.__state._region:  # pylint: disable=protected-access
                try:
                    async with self.__state._lock:  # pylint: disable=protected-access
                        await self.__changes_queue.put(None)
                        assert self.__state.storage
                        assert self.__state.vd

                        if self.__state.vd.connected or self.__drive.get_image_path():
                            raise MsdConnectedError()

                        path = os.path.join(self.__images_path, name)
                        if name in self.__state.storage.images or os.path.exists(path):
                            raise MsdImageExistsError()

                        await self.__remount_storage(rw=True)
                        self.__set_image_complete(name, False)
                        self.__new_file_written = 0
                        self.__new_file = await aiofiles.open(path, mode="w+b", buffering=0)

                    await self.__changes_queue.put(None)
                    yield
                    self.__set_image_complete(name, True)

                finally:
                    await self.__close_new_file()
                    try:
                        await self.__remount_storage(rw=False)
                    except asyncio.CancelledError:  # pylint: disable=try-except-raise
                        raise
                    except Exception:
                        pass
        finally:
            await self.__changes_queue.put(None)

    @aiotools.atomic
    async def write_image_chunk(self, chunk: bytes) -> int:
        assert self.__new_file
        await aiotools.afile_write_now(self.__new_file, chunk)
        self.__new_file_written += len(chunk)
        return self.__new_file_written

    async def remove(self, name: str) -> None:
        async with self.__state.busy():
            assert self.__state.storage
            assert self.__state.vd

            if self.__state.vd.connected or self.__drive.get_image_path():
                raise MsdConnectedError()

            image = self.__state.storage.images.get(name)
            if image is None or not os.path.exists(image.path):
                raise MsdUnknownImageError()
            assert image.in_storage

            if self.__state.vd.image == image:
                self.__state.vd.image = None
            del self.__state.storage.images[name]

            await self.__remount_storage(rw=True)
            os.remove(image.path)
            self.__set_image_complete(name, False)
            await self.__remount_storage(rw=False)

    # =====

    async def __close_new_file(self) -> None:
        try:
            if self.__new_file:
                get_logger().info("Closing new image file ...")
                await self.__new_file.close()
        except asyncio.CancelledError:  # pylint: disable=try-except-raise
            raise
        except Exception:
            get_logger().exception("Can't close device file")
        finally:
            self.__new_file = None
            self.__new_file_written = 0

    # =====

    async def __watch_inotify(self) -> None:
        logger = get_logger(0)
        while True:
            try:
                while True:
                    # Активно ждем, пока не будут на месте все каталоги.
                    await self.__reload_state()
                    await self.__changes_queue.put(None)
                    if self.__state.vd:
                        break
                    await asyncio.sleep(5)

                with Inotify() as inotify:
                    inotify.watch(self.__images_path, InotifyMask.ALL_MODIFY_EVENTS)
                    inotify.watch(self.__meta_path, InotifyMask.ALL_MODIFY_EVENTS)
                    inotify.watch(self.__drive.get_sysfs_path(), InotifyMask.ALL_MODIFY_EVENTS)

                    # После установки вотчеров еще раз проверяем стейт, чтобы ничего не потерять
                    await self.__reload_state()
                    await self.__changes_queue.put(None)

                    while self.__state.vd:  # Если живы после предыдущей проверки
                        need_restart = False
                        need_reload_state = False
                        for event in (await inotify.get_series(timeout=1)):
                            need_reload_state = True
                            if event.mask & (InotifyMask.DELETE_SELF | InotifyMask.MOVE_SELF | InotifyMask.UNMOUNT):
                                # Если выгрузили OTG, что-то отмонтировали или делают еще какую-то странную фигню
                                logger.warning("Got fatal inotify event: %s; reinitializing MSD ...", event)
                                need_restart = True
                                break
                        if need_restart:
                            break
                        if need_reload_state:
                            await self.__reload_state()
                            await self.__changes_queue.put(None)
            except asyncio.CancelledError:  # pylint: disable=try-except-raise
                raise
            except Exception:
                logger.exception("Unexpected MSD watcher error")

    async def __reload_state(self) -> None:
        logger = get_logger(0)
        async with self.__state._lock:  # pylint: disable=protected-access
            try:
                drive_state = self.__get_drive_state()
                if drive_state.rw:
                    # Внештатное использование MSD, ломаемся
                    raise MsdError("MSD has been switched to RW-mode manually")

                if self.__state.vd is None and drive_state.image is None:
                    # Если только что включились и образ не подключен - попробовать
                    # перемонтировать хранилище (и создать images и meta).
                    logger.info("Probing to remount storage ...")
                    await self.__remount_storage(rw=True)
                    await self.__remount_storage(rw=False)

                storage_state = self.__get_storage_state()
            except asyncio.CancelledError:  # pylint: disable=try-except-raise
                raise
            except Exception:
                logger.exception("Error while reloading MSD state; switching to offline")
                self.__state.storage = None
                self.__state.vd = None
            else:
                self.__state.storage = storage_state
                if drive_state.image:
                    # При подключенном образе виртуальный стейт заменяется реальным
                    self.__state.vd = _VirtualDriveState.from_drive_state(drive_state)
                else:
                    if self.__state.vd is None:
                        # Если раньше MSD был отключен
                        self.__state.vd = _VirtualDriveState.from_drive_state(drive_state)

                    if (
                        self.__state.vd.image
                        and (not self.__state.vd.image.in_storage or not os.path.exists(self.__state.vd.image.path))
                    ):
                        # Если только что отключили ручной образ вне хранилища или ранее выбранный образ был удален
                        self.__state.vd.image = None

                    self.__state.vd.connected = False

    # =====

    def __get_storage_state(self) -> _StorageState:
        images: Dict[str, _DriveImage] = {}
        for name in os.listdir(self.__images_path):
            path = os.path.join(self.__images_path, name)
            if os.path.exists(path):
                size = self.__get_file_size(path)
                if size >= 0:
                    images[name] = _DriveImage(
                        name=name,
                        path=path,
                        size=size,
                        complete=self.__is_image_complete(name),
                        in_storage=True,
                    )
        st = os.statvfs(self.__storage_path)
        return _StorageState(
            size=(st.f_blocks * st.f_frsize),
            free=(st.f_bavail * st.f_frsize),
            images=images,
        )

    def __get_drive_state(self) -> _DriveState:
        image: Optional[_DriveImage] = None
        path = self.__drive.get_image_path()
        if path:
            name = os.path.basename(path)
            in_storage = (os.path.dirname(path) == self.__images_path)
            image = _DriveImage(
                name=name,
                path=path,
                size=max(self.__get_file_size(path), 0),
                complete=(self.__is_image_complete(name) if in_storage else True),
                in_storage=in_storage,
            )
        return _DriveState(
            image=image,
            cdrom=self.__drive.get_cdrom_flag(),
            rw=self.__drive.get_rw_flag(),
        )

    # =====

    def __get_file_size(self, path: str) -> int:
        try:
            return os.path.getsize(path)
        except Exception as err:
            get_logger().warning("Can't get size of file %s: %s", path, err)
            return -1

    def __is_image_complete(self, name: str) -> bool:
        return os.path.exists(os.path.join(self.__meta_path, name + ".complete"))

    def __set_image_complete(self, name: str, flag: bool) -> None:
        path = os.path.join(self.__meta_path, name + ".complete")
        if flag:
            open(path, "w").close()
        else:
            if os.path.exists(path):
                os.remove(path)

    # =====

    async def __remount_storage(self, rw: bool) -> None:
        await remount_storage(self.__remount_cmd, rw)

    async def __unlock_drive(self) -> None:
        await unlock_drive(self.__unlock_cmd)
