# ========================================================================== #
#                                                                            #
#    KVMD - The main PiKVM daemon.                                           #
#                                                                            #
#    Copyright (C) 2018-2022  Maxim Devaev <mdevaev@gmail.com>               #
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
import functools
import time

from typing import List
from typing import Dict
from typing import AsyncGenerator
from typing import Optional

from ....logging import get_logger

from ....inotify import InotifyMask
from ....inotify import Inotify

from ....yamlconf import Option

from ....validators.basic import valid_bool
from ....validators.basic import valid_number
from ....validators.os import valid_abs_dir
from ....validators.os import valid_printable_filename
from ....validators.os import valid_command

from .... import aiotools
from .... import aiohelpers

from .. import MsdError
from .. import MsdIsBusyError
from .. import MsdOfflineError
from .. import MsdConnectedError
from .. import MsdDisconnectedError
from .. import MsdImageNotSelected
from .. import MsdUnknownImageError
from .. import MsdImageExistsError
from .. import BaseMsd
from .. import MsdFileReader
from .. import MsdFileWriter

from . import fs

from .drive import Drive


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
    rw: bool

    @classmethod
    def from_drive_state(cls, state: _DriveState) -> "_VirtualDriveState":
        return _VirtualDriveState(
            image=state.image,
            connected=bool(state.image),
            cdrom=state.cdrom,
            rw=state.rw,
        )


class _State:
    def __init__(self, notifier: aiotools.AioNotifier) -> None:
        self.__notifier = notifier

        self.storage: Optional[_StorageState] = None
        self.vd: Optional[_VirtualDriveState] = None

        self._lock = asyncio.Lock()
        self._region = aiotools.AioExclusiveRegion(MsdIsBusyError)

    @contextlib.asynccontextmanager
    async def busy(self, check_online: bool=True) -> AsyncGenerator[None, None]:
        async with self._region:
            async with self._lock:
                self.__notifier.notify()
                if check_online:
                    if self.vd is None:
                        raise MsdOfflineError()
                    assert self.storage
                yield
        self.__notifier.notify()

    def is_busy(self) -> bool:
        return self._region.is_busy()


# =====
class Plugin(BaseMsd):  # pylint: disable=too-many-instance-attributes
    def __init__(  # pylint: disable=super-init-not-called
        self,
        read_chunk_size: int,
        write_chunk_size: int,
        sync_chunk_size: int,

        storage_path: str,

        remount_cmd: List[str],

        initial: Dict,

        gadget: str,  # XXX: Not from options, see /kvmd/apps/kvmd/__init__.py for details
    ) -> None:

        self.__read_chunk_size = read_chunk_size
        self.__write_chunk_size = write_chunk_size
        self.__sync_chunk_size = sync_chunk_size

        self.__storage_path = os.path.normpath(storage_path)
        self.__images_path = os.path.join(self.__storage_path, "images")
        self.__meta_path = os.path.join(self.__storage_path, "meta")

        self.__remount_cmd = remount_cmd

        self.__initial_image: str = initial["image"]
        self.__initial_cdrom: bool = initial["cdrom"]

        self.__drive = Drive(gadget, instance=0, lun=0)

        self.__reader: Optional[MsdFileReader] = None
        self.__writer: Optional[MsdFileWriter] = None

        self.__notifier = aiotools.AioNotifier()
        self.__state = _State(self.__notifier)

        logger = get_logger(0)
        logger.info("Using OTG gadget %r as MSD", gadget)
        aiotools.run_sync(self.__reload_state(notify=False))

    @classmethod
    def get_plugin_options(cls) -> Dict:
        return {
            "read_chunk_size":   Option(65536,   type=functools.partial(valid_number, min=1024)),
            "write_chunk_size":  Option(65536,   type=functools.partial(valid_number, min=1024)),
            "sync_chunk_size":   Option(4194304, type=functools.partial(valid_number, min=1024)),

            "storage": Option("/var/lib/kvmd/msd", type=valid_abs_dir, unpack_as="storage_path"),

            "remount_cmd": Option([
                "/usr/bin/sudo", "--non-interactive",
                "/usr/bin/kvmd-helper-otgmsd-remount", "{mode}",
            ], type=valid_command),

            "initial": {
                "image": Option("",    type=valid_printable_filename, if_empty=""),
                "cdrom": Option(False, type=valid_bool),
            },
        }

    async def get_state(self) -> Dict:
        async with self.__state._lock:  # pylint: disable=protected-access
            storage: Optional[Dict] = None
            if self.__state.storage:
                storage = dataclasses.asdict(self.__state.storage)
                for name in list(storage["images"]):
                    del storage["images"][name]["path"]
                    del storage["images"][name]["in_storage"]

                storage["downloading"] = (self.__reader.get_state() if self.__reader else None)

                if self.__writer:
                    # При загрузке файла показываем актуальную статистику вручную
                    storage["uploading"] = self.__writer.get_state()
                    space = fs.get_fs_space(self.__storage_path, fatal=False)
                    if space:
                        storage.update(dataclasses.asdict(space))
                else:
                    storage["uploading"] = None

            vd: Optional[Dict] = None
            if self.__state.vd:
                vd = dataclasses.asdict(self.__state.vd)
                if vd["image"]:
                    del vd["image"]["path"]

            return {
                "enabled": True,
                "online": (bool(self.__state.vd) and self.__drive.is_enabled()),
                "busy": self.__state.is_busy(),
                "storage": storage,
                "drive": vd,
                "features": {
                    "multi": True,
                    "cdrom": True,
                    "rw": True,
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

    async def systask(self) -> None:
        await self.__watch_inotify()

    @aiotools.atomic
    async def reset(self) -> None:
        async with self.__state.busy(check_online=False):
            try:
                self.__drive.set_image_path("")
                self.__drive.set_cdrom_flag(False)
                self.__drive.set_rw_flag(False)
                await self.__remount_rw(False)
            except Exception:
                get_logger(0).exception("Can't reset MSD properly")

    @aiotools.atomic
    async def cleanup(self) -> None:
        await self.__close_reader()
        await self.__close_writer()

    # =====

    @aiotools.atomic
    async def set_params(
        self,
        name: Optional[str]=None,
        cdrom: Optional[bool]=None,
        rw: Optional[bool]=None,
    ) -> None:

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
                if cdrom:
                    rw = False

            if rw is not None:
                self.__state.vd.rw = rw
                if rw:
                    self.__state.vd.cdrom = False

    @aiotools.atomic
    async def set_connected(self, connected: bool) -> None:
        async with self.__state.busy():
            assert self.__state.vd
            if connected:
                if self.__state.vd.connected or self.__drive.get_image_path():
                    raise MsdConnectedError()
                if self.__state.vd.image is None:
                    raise MsdImageNotSelected()

                assert self.__state.vd.image.in_storage

                if not os.path.exists(self.__state.vd.image.path):
                    raise MsdUnknownImageError()

                self.__drive.set_rw_flag(self.__state.vd.rw)
                self.__drive.set_cdrom_flag(self.__state.vd.cdrom)
                if self.__state.vd.rw:
                    await self.__remount_rw(True)
                self.__drive.set_image_path(self.__state.vd.image.path)

            else:
                if not (self.__state.vd.connected or self.__drive.get_image_path()):
                    raise MsdDisconnectedError()
                self.__drive.set_image_path("")
                await self.__remount_rw(False, fatal=False)

            self.__state.vd.connected = connected

    @contextlib.asynccontextmanager
    async def read_image(self, name: str) -> AsyncGenerator[MsdFileReader, None]:
        try:
            async with self.__state._region:  # pylint: disable=protected-access
                try:
                    async with self.__state._lock:  # pylint: disable=protected-access
                        self.__notifier.notify()
                        assert self.__state.storage
                        assert self.__state.vd

                        if self.__state.vd.connected or self.__drive.get_image_path():
                            raise MsdConnectedError()

                        path = os.path.join(self.__images_path, name)
                        if name not in self.__state.storage.images or not os.path.exists(path):
                            raise MsdUnknownImageError()

                        self.__reader = await MsdFileReader(
                            notifier=self.__notifier,
                            path=path,
                            chunk_size=self.__read_chunk_size,
                        ).open()

                    yield self.__reader

                finally:
                    # FIXME: Перехват важен потому что по какой-то причине await
                    # во вложенных finally путаются и выполняются не по порядку
                    try:
                        await asyncio.shield(self.__close_reader())
                    except asyncio.CancelledError:
                        pass
        finally:
            self.__notifier.notify()

    @contextlib.asynccontextmanager
    async def write_image(self, name: str, size: int, remove_incomplete: Optional[bool]) -> AsyncGenerator[MsdFileWriter, None]:
        try:
            async with self.__state._region:  # pylint: disable=protected-access
                path: str = ""
                try:
                    async with self.__state._lock:  # pylint: disable=protected-access
                        self.__notifier.notify()
                        assert self.__state.storage
                        assert self.__state.vd

                        if self.__state.vd.connected or self.__drive.get_image_path():
                            raise MsdConnectedError()

                        path = os.path.join(self.__images_path, name)
                        if name in self.__state.storage.images or os.path.exists(path):
                            raise MsdImageExistsError()

                        await self.__remount_rw(True)
                        self.__set_image_complete(name, False)

                        self.__writer = await MsdFileWriter(
                            notifier=self.__notifier,
                            path=path,
                            file_size=size,
                            sync_size=self.__sync_chunk_size,
                            chunk_size=self.__write_chunk_size,
                        ).open()

                    self.__notifier.notify()
                    yield self.__writer
                    self.__set_image_complete(name, self.__writer.is_complete())

                finally:
                    if remove_incomplete and self.__writer and not self.__writer.is_complete():
                        # Можно сперва удалить файл, потом закрыть его
                        try:
                            os.remove(path)
                        except Exception:
                            pass
                    try:
                        await asyncio.shield(self.__close_writer())
                    except asyncio.CancelledError:
                        pass
                    try:
                        await asyncio.shield(self.__remount_rw(False, fatal=False))
                    except asyncio.CancelledError:
                        pass
        finally:
            # Между закрытием файла и эвентом айнотифи состояние может быть не обновлено,
            # так что форсим обновление вручную, чтобы получить актуальное состояние.
            try:
                await asyncio.shield(self.__reload_state())
            except asyncio.CancelledError:
                pass

    @aiotools.atomic
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

            await self.__remount_rw(True)
            os.remove(image.path)
            self.__set_image_complete(name, False)
            await self.__remount_rw(False, fatal=False)

    # =====

    async def __close_reader(self) -> None:
        if self.__reader:
            await self.__reader.close()
            self.__reader = None

    async def __close_writer(self) -> None:
        if self.__writer:
            await self.__writer.close()
            self.__writer = None

    # =====

    async def __watch_inotify(self) -> None:
        logger = get_logger(0)
        while True:
            try:
                while True:
                    # Активно ждем, пока не будут на месте все каталоги.
                    await self.__reload_state()
                    if self.__state.vd:
                        break
                    await asyncio.sleep(5)

                with Inotify() as inotify:
                    inotify.watch(self.__images_path, InotifyMask.ALL_MODIFY_EVENTS)
                    inotify.watch(self.__meta_path, InotifyMask.ALL_MODIFY_EVENTS)
                    for path in self.__drive.get_watchable_paths():
                        inotify.watch(path, InotifyMask.ALL_MODIFY_EVENTS)

                    # После установки вотчеров еще раз проверяем стейт, чтобы ничего не потерять
                    await self.__reload_state()

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
            except Exception:
                logger.exception("Unexpected MSD watcher error")
                time.sleep(1)

    async def __reload_state(self, notify: bool=True) -> None:
        logger = get_logger(0)
        async with self.__state._lock:  # pylint: disable=protected-access
            try:
                drive_state = self.__get_drive_state()

                if self.__state.vd is None and drive_state.image is None:
                    # Если только что включились и образ не подключен - попробовать
                    # перемонтировать хранилище (и создать images и meta).
                    logger.info("Probing to remount storage ...")
                    await self.__remount_rw(True)
                    await self.__remount_rw(False)
                    await self.__setup_initial()

                storage_state = self.__get_storage_state()
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
        if notify:
            self.__notifier.notify()

    async def __setup_initial(self) -> None:
        if self.__initial_image:
            logger = get_logger(0)
            path = os.path.join(self.__images_path, self.__initial_image)
            if os.path.exists(path):
                logger.info("Setting up initial image %r ...", self.__initial_image)
                try:
                    self.__drive.set_rw_flag(False)
                    self.__drive.set_cdrom_flag(self.__initial_cdrom)
                    self.__drive.set_image_path(path)
                except Exception:
                    logger.exception("Can't setup initial image: ignored")
            else:
                logger.error("Can't find initial image %r: ignored", self.__initial_image)

    # =====

    def __get_storage_state(self) -> _StorageState:
        images: Dict[str, _DriveImage] = {}
        for name in os.listdir(self.__images_path):
            path = os.path.join(self.__images_path, name)
            if os.path.exists(path):
                size = fs.get_file_size(path)
                if size >= 0:
                    images[name] = _DriveImage(
                        name=name,
                        path=path,
                        size=size,
                        complete=self.__is_image_complete(name),
                        in_storage=True,
                    )
        space = fs.get_fs_space(self.__storage_path, fatal=True)
        assert space
        return _StorageState(
            size=space.size,
            free=space.free,
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
                size=max(fs.get_file_size(path), 0),
                complete=(self.__is_image_complete(name) if in_storage else True),
                in_storage=in_storage,
            )
        return _DriveState(
            image=image,
            cdrom=self.__drive.get_cdrom_flag(),
            rw=self.__drive.get_rw_flag(),
        )

    # =====

    def __is_image_complete(self, name: str) -> bool:
        return os.path.exists(os.path.join(self.__meta_path, name + ".complete"))

    def __set_image_complete(self, name: str, flag: bool) -> None:
        path = os.path.join(self.__meta_path, name + ".complete")
        if flag:
            open(path, "w").close()  # pylint: disable=consider-using-with
        else:
            if os.path.exists(path):
                os.remove(path)

    # =====

    async def __remount_rw(self, rw: bool, fatal: bool=True) -> None:
        if not (await aiohelpers.remount("MSD", self.__remount_cmd, rw)):
            if fatal:
                raise MsdError("Can't execute remount helper")
