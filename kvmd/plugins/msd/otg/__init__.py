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


import asyncio
import contextlib
import dataclasses
import functools
import time
import os
import copy
import pyfatfs
import pyfatfs.PyFat
import pyfatfs.PyFatFS

from typing import AsyncGenerator

from ....logging import get_logger

from ....inotify import Inotify

from ....yamlconf import Option

from ....validators.basic import valid_bool
from ....validators.basic import valid_number, valid_stripped_string_not_empty
from ....validators.os import valid_command, valid_abs_path
from ....validators.kvm import valid_msd_image_name

from .... import aiotools
from .... import fstab

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

from .storage import Image
from .storage import Storage
from .drive import Drive


# =====
@dataclasses.dataclass(frozen=True)
class _DriveState:
    image: (Image | None)
    cdrom: bool
    rw: bool


@dataclasses.dataclass
class _VirtualDriveState:
    image: (Image | None)
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

        self.storage: (Storage | None) = None
        self.vd: (_VirtualDriveState | None) = None

        self._region = aiotools.AioExclusiveRegion(MsdIsBusyError)
        self._lock = asyncio.Lock()

    @contextlib.asynccontextmanager
    async def busy(self, check_online: bool=True) -> AsyncGenerator[None, None]:
        try:
            with self._region:
                async with self._lock:
                    self.__notifier.notify()
                    if check_online:
                        if self.vd is None:
                            raise MsdOfflineError()
                        assert self.storage
                    yield
        finally:
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
        normalfiles_size: int,

        remount_cmd: list[str],

        initial: dict,

        normalfiles_path: str,
        msd_path: str,

        gadget: str,  # XXX: Not from options, see /kvmd/apps/kvmd/__init__.py for details
    ) -> None:

        self.__read_chunk_size = read_chunk_size
        self.__write_chunk_size = write_chunk_size
        self.__sync_chunk_size = sync_chunk_size
        self.__normalfiles_path = normalfiles_path
        self.__msd_path = msd_path
        self.__normalfiles_size = normalfiles_size

        self.__initial_image: str = initial["image"]
        self.__initial_cdrom: bool = initial["cdrom"]

        self.__drive = Drive(gadget, instance=0, lun=0)
        self.__storage = Storage(fstab.find_msd(msd_path).root_path, remount_cmd)

        self.__reader: (MsdFileReader | None) = None
        self.__writer: (MsdFileWriter | None) = None

        self.__notifier = aiotools.AioNotifier()
        self.__state = _State(self.__notifier)
        self.__reset = False

        logger = get_logger(0)
        logger.info("Using OTG gadget %r as MSD", gadget)
        aiotools.run_sync(self.__unsafe_reload_state())

    @classmethod
    def get_plugin_options(cls) -> dict:
        return {
            "read_chunk_size":   Option(65536,   type=functools.partial(valid_number, min=1024)),
            "write_chunk_size":  Option(65536,   type=functools.partial(valid_number, min=1024)),
            "sync_chunk_size":   Option(4194304, type=functools.partial(valid_number, min=1024)),

            "remount_cmd": Option([
                "/usr/bin/sudo", "--non-interactive",
                "/usr/bin/kvmd-helper-otgmsd-remount", "{mode}",
            ], type=valid_command),

            "initial": {
                "image": Option("",    type=valid_msd_image_name, if_empty=""),
                "cdrom": Option(False, type=valid_bool),
            },
            "msd_path": Option("/var/lib/kvmd/msd",    type=valid_abs_path),
            "normalfiles_path": Option("NormalFiles", type=valid_stripped_string_not_empty),
            "normalfiles_size":   Option(256,   type=functools.partial(valid_number, min=64)),
        }

    # =====

    # =====

    async def get_state(self) -> dict:
        async with self.__state._lock:  # pylint: disable=protected-access
            storage: (dict | None) = None
            if self.__state.storage:
                assert self.__state.vd
                storage = dataclasses.asdict(self.__state.storage)
                for name in list(storage["images"]):
                    del storage["images"][name]["name"]
                    del storage["images"][name]["path"]
                    del storage["images"][name]["in_storage"]
                for name in list(storage["parts"]):
                    del storage["parts"][name]["name"]

                storage["downloading"] = (self.__reader.get_state() if self.__reader else None)
                storage["uploading"] = (self.__writer.get_state() if self.__writer else None)
                storage["filespath"] = self.__normalfiles_path

            vd: (dict | None) = None
            if self.__state.vd:
                assert self.__state.storage
                vd = dataclasses.asdict(self.__state.vd)
                if vd["image"]:
                    del vd["image"]["path"]

            return {
                "enabled": True,
                "online": (bool(vd) and self.__drive.is_enabled()),
                "busy": self.__state.is_busy(),
                "storage": storage,
                "drive": vd,
            }

    async def trigger_state(self) -> None:
        self.__notifier.notify(1)

    async def poll_state(self) -> AsyncGenerator[dict, None]:
        prev: dict = {}
        while True:
            if (await self.__notifier.wait()) > 0:
                prev = {}
            new = await self.get_state()
            if not prev or (prev.get("online") != new["online"]):
                prev = copy.deepcopy(new)
                yield new
            else:
                diff: dict = {}
                for sub in ["busy", "drive"]:
                    if prev.get(sub) != new[sub]:
                        diff[sub] = new[sub]
                for sub in ["images", "parts", "downloading", "uploading"]:
                    if (prev.get("storage") or {}).get(sub) != (new["storage"] or {}).get(sub):
                        if "storage" not in diff:
                            diff["storage"] = {}
                        diff["storage"][sub] = new["storage"][sub]
                if diff:
                    prev = copy.deepcopy(new)
                    yield diff

    @aiotools.atomic_fg
    async def reset(self) -> None:
        async with self.__state.busy(check_online=False):
            try:
                self.__reset = True
                self.__drive.set_image_path("")
                self.__drive.set_cdrom_flag(False)
                self.__drive.set_rw_flag(False)
                await self.__storage.remount_rw(False)
            except Exception:
                get_logger(0).exception("Can't reset MSD properly")

    # =====

    @aiotools.atomic_fg
    async def set_params(
        self,
        name: (str | None)=None,
        cdrom: (bool | None)=None,
        rw: (bool | None)=None,
    ) -> None:

        async with self.__state.busy():
            assert self.__state.vd
            self.__STATE_check_disconnected()

            if name is not None:
                if name:
                    self.__state.vd.image = await self.__STATE_get_storage_image(name)
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

    @aiotools.atomic_fg
    async def set_connected(self, connected: bool) -> None:
        print(self.__drive)
        async with self.__state.busy():
            assert self.__state.vd
            if connected:
                self.__STATE_check_disconnected()

                if self.__state.vd.image is None:
                    raise MsdImageNotSelected()

                if not (await self.__state.vd.image.exists()):
                    raise MsdUnknownImageError()

                assert self.__state.vd.image.in_storage

                self.__drive.set_rw_flag(self.__state.vd.rw)
                self.__drive.set_cdrom_flag(self.__state.vd.cdrom)
                #reset UDC to fix otg cd-rom and flash switch
                try:
                    udc_path = self.__drive.get_udc_path()
                    with open(udc_path) as file:
                        enabled = bool(file.read().strip())
                    if enabled:
                        with open(udc_path, "w") as file:
                            file.write("\n")
                    with open(udc_path, "w") as file:
                        file.write(sorted(os.listdir("/sys/class/udc"))[0])
                except:
                    logger = get_logger(0)
                    logger.error("Can't reset UDC")
                if self.__state.vd.rw:
                    await self.__state.vd.image.remount_rw(True)
                self.__drive.set_image_path(self.__state.vd.image.path)

            else:
                self.__STATE_check_connected()
                self.__drive.set_image_path("")
                await self.__storage.remount_rw(False, fatal=False)

            self.__state.vd.connected = connected

    @aiotools.atomic_fg
    async def make_image(self, zipped: bool) -> None:
        #Note: img size >= 64M
        def create_fat_image(img_size: int, file_img_path: str, source_dir: str, fat_type: int = 32, label: str = 'One-KVM'):
            def add_directory_to_fat(fat: str, src_path: str, dst_path: str):
                for item in os.listdir(src_path):
                    src_item_path = os.path.join(src_path, item)
                    dst_item_path = os.path.join(dst_path, item)

                    if os.path.isdir(src_item_path):
                        fat.makedir(dst_item_path)
                        add_directory_to_fat(fat, src_item_path, dst_item_path)
                    elif os.path.isfile(src_item_path):
                        with open(src_item_path, 'rb') as src_file:
                            fat.create(dst_item_path)
                            with fat.open(dst_item_path, 'wb') as dst_file:
                                dst_file.write(src_file.read())
            print(file_img_path)
            with open(file_img_path, 'wb') as f:
                f.seek(img_size * 1024 *1024 - 1)
                f.write(b'\0')
            fat_file =  pyfatfs.PyFat.PyFat()
            try:
                fat_file.mkfs(file_img_path, fat_type = fat_type, label = label)
            except Exception as e:
                get_logger(0).exception(f"Error making FAT Filesystem: {e}")
            finally:
                fat_file.close()
            fat_handle =  pyfatfs.PyFatFS.PyFatFS(file_img_path)
            try:
                add_directory_to_fat(fat_handle, source_dir, '/')
            except Exception as e:
                get_logger(0).exception(f"Error adding directory to FAT image: {e}")
            finally:
                fat_handle.close()

        def extract_fat_image(file_img_path: str, output_dir: str):
            try:
                for root, dirs, files in os.walk(output_dir, topdown=False):
                    for name in files:
                        os.remove(os.path.join(root, name))
                    for name in dirs:
                        os.rmdir(os.path.join(root, name))
            except Exception as e:
                get_logger(0).exception(f"Error removing normal file or directory: {e}")
            fat_handle =  pyfatfs.PyFatFS.PyFatFS(file_img_path)
            try:
                def extract_directory(fat_handle, src_path: str, dst_path: str):
                    for entry in fat_handle.listdir(src_path):
                        src_item_path = os.path.join(src_path, entry)
                        dst_item_path = os.path.join(dst_path, entry)

                        if fat_handle.gettype(src_item_path) is pyfatfs.PyFatFS.ResourceType.directory:
                            os.makedirs(dst_item_path, exist_ok=True)
                            extract_directory(fat_handle, src_item_path, dst_item_path)
                        else:
                            with fat_handle.open(src_item_path, 'rb') as src_file:
                                with open(dst_item_path, 'wb') as dst_file:
                                    dst_file.write(src_file.read())
                extract_directory(fat_handle, '/', output_dir)
            except Exception as e:
                get_logger(0).exception(f"Error extracting FAT image: {e}")
            finally:
                fat_handle.close()

        async with self.__state.busy():
            msd_path = self.__msd_path
            file_storage_path = os.path.join(msd_path, self.__normalfiles_path)
            file_img_path = os.path.join(msd_path, self.__normalfiles_path + ".img")
            img_size = self.__normalfiles_size
            if zipped:
                if not os.path.exists(file_storage_path):
                    os.makedirs(file_storage_path)
                if os.path.exists(file_img_path):
                    os.remove(file_img_path)
                create_fat_image(img_size, file_img_path, file_storage_path)
            else:
                if os.path.exists(file_img_path):
                    extract_fat_image(file_img_path, file_storage_path)

    @contextlib.asynccontextmanager
    async def read_image(self, name: str) -> AsyncGenerator[MsdFileReader, None]:
        try:
            with self.__state._region:  # pylint: disable=protected-access
                try:
                    async with self.__state._lock:  # pylint: disable=protected-access
                        self.__notifier.notify()
                        self.__STATE_check_disconnected()

                        image = await self.__STATE_get_storage_image(name)
                        self.__reader = await MsdFileReader(
                            notifier=self.__notifier,
                            name=image.name,
                            path=image.path,
                            chunk_size=self.__read_chunk_size,
                        ).open()

                    self.__notifier.notify()
                    yield self.__reader

                finally:
                    await aiotools.shield_fg(self.__close_reader())
        finally:
            self.__notifier.notify()

    @contextlib.asynccontextmanager
    async def write_image(self, name: str, size: int, remove_incomplete: (bool | None)) -> AsyncGenerator[MsdFileWriter, None]:
        image: (Image | None) = None
        complete = False

        async def finish_writing() -> None:
            # Делаем под блокировкой, чтобы эвент айнотифи не был обработан
            # до того, как мы не закончим все процедуры.
            async with self.__state._lock:  # pylint: disable=protected-access
                try:
                    self.__notifier.notify()
                finally:
                    try:
                        if image:
                            await image.set_complete(complete)
                    finally:
                        try:
                            if image and remove_incomplete and not complete:
                                await image.remove(fatal=False)
                        finally:
                            try:
                                await self.__close_writer()
                            finally:
                                if image:
                                    await image.remount_rw(False, fatal=False)

        try:
            with self.__state._region:  # pylint: disable=protected-access
                try:
                    async with self.__state._lock:  # pylint: disable=protected-access
                        self.__notifier.notify()
                        self.__STATE_check_disconnected()

                        image = await self.__STORAGE_create_new_image(name)
                        await image.remount_rw(True)
                        await image.set_complete(False)
                        self.__writer = await MsdFileWriter(
                            notifier=self.__notifier,
                            name=image.name,
                            path=image.path,
                            file_size=size,
                            sync_size=self.__sync_chunk_size,
                            chunk_size=self.__write_chunk_size,
                        ).open()

                    self.__notifier.notify()
                    yield self.__writer
                    complete = await self.__writer.finish()

                finally:
                    await aiotools.shield_fg(finish_writing())
        finally:
            self.__notifier.notify()

    @aiotools.atomic_fg
    async def remove(self, name: str) -> None:
        async with self.__state.busy():
            assert self.__state.storage
            assert self.__state.vd
            self.__STATE_check_disconnected()
            image = await self.__STATE_get_storage_image(name)

            if self.__state.vd.image == image:
                self.__state.vd.image = None

            await image.remount_rw(True)
            try:
                await image.remove(fatal=True)
            finally:
                await aiotools.shield_fg(image.remount_rw(False, fatal=False))

    # =====

    def __STATE_check_connected(self) -> None:  # pylint: disable=invalid-name
        assert self.__state.vd
        if not (self.__state.vd.connected or self.__drive.get_image_path()):
            raise MsdDisconnectedError()

    def __STATE_check_disconnected(self) -> None:  # pylint: disable=invalid-name
        assert self.__state.vd
        if self.__state.vd.connected or self.__drive.get_image_path():
            raise MsdConnectedError()

    async def __STATE_get_storage_image(self, name: str) -> Image:  # pylint: disable=invalid-name
        assert self.__state.storage
        image = self.__state.storage.images.get(name)
        if image is None or not (await image.exists()):
            raise MsdUnknownImageError()
        assert image.in_storage
        return image

    async def __STORAGE_create_new_image(self, name: str) -> Image:  # pylint: disable=invalid-name
        assert self.__state.storage
        image = await self.__storage.make_image_by_name(name)
        if image.name in self.__state.storage.images or (await image.exists()):
            raise MsdImageExistsError()
        return image

    # =====

    async def __close_reader(self) -> None:
        if self.__reader:
            try:
                await self.__reader.close()
            finally:
                self.__reader = None

    async def __close_writer(self) -> None:
        if self.__writer:
            try:
                await self.__writer.close()
            finally:
                self.__writer = None

    # =====

    @aiotools.atomic_fg
    async def cleanup(self) -> None:
        await self.__close_reader()
        await self.__close_writer()

    async def systask(self) -> None:
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
                    # Из-за гонки между первым релоадом и установкой вотчеров,
                    # мы можем потерять какие-то каталоги стораджа, но это допустимо,
                    # так как всегда есть ручной перезапуск.
                    await inotify.watch_all_changes(*self.__storage.get_watchable_paths())
                    await inotify.watch_all_changes(*self.__drive.get_watchable_paths())

                    # После установки вотчеров еще раз проверяем стейт,
                    # чтобы не потерять состояние привода.
                    await self.__reload_state()

                    while self.__state.vd:  # Если живы после предыдущей проверки
                        need_restart = self.__reset
                        self.__reset = False
                        need_reload_state = False
                        for event in (await inotify.get_series(timeout=1)):
                            need_reload_state = True
                            if event.restart:
                                # Если выгрузили OTG, изменили каталоги, что-то отмонтировали или делают еще какую-то странную фигню.
                                # Проверяется маска InotifyMask.ALL_RESTART_EVENTS
                                logger.info("Got a big inotify event: %s; reinitializing MSD ...", event)
                                need_restart = True
                                break
                        if need_restart:
                            break
                        if need_reload_state:
                            await self.__reload_state()
                        elif self.__writer:
                            # При загрузке файла обновляем статистику раз в секунду (по таймауту).
                            # Это не нужно при обычном релоаде, потому что там и так проверяются все разделы.
                            await self.__reload_parts_info()

            except Exception:
                logger.exception("Unexpected MSD watcher error")
                await asyncio.sleep(1)

    async def __reload_state(self) -> None:
        async with self.__state._lock:  # pylint: disable=protected-access
            await self.__unsafe_reload_state()
        self.__notifier.notify()

    async def __reload_parts_info(self) -> None:
        assert self.__writer  # Использовать только при записи образа
        async with self.__state._lock:  # pylint: disable=protected-access
            await self.__storage.reload_parts_info()
        self.__notifier.notify()

    # ===== Don't call this directly ====

    async def __unsafe_reload_state(self) -> None:
        logger = get_logger(0)
        try:
            path = self.__drive.get_image_path()
            drive_state = _DriveState(
                image=((await self.__storage.make_image_by_path(path)) if path else None),
                cdrom=self.__drive.get_cdrom_flag(),
                rw=self.__drive.get_rw_flag(),
            )

            await self.__storage.reload()

            if self.__state.vd is None and drive_state.image is None:
                # Если только что включились и образ не подключен - попробовать
                # перемонтировать хранилище (и создать images и meta).
                logger.info("Probing to remount storage ...")
                await self.__storage.remount_rw(True)
                await self.__storage.remount_rw(False)
                await self.__unsafe_setup_initial()

        except Exception:
            logger.exception("Error while reloading MSD state; switching to offline")
            self.__state.storage = None
            self.__state.vd = None

        else:
            self.__state.storage = self.__storage
            if drive_state.image:
                # При подключенном образе виртуальный стейт заменяется реальным
                self.__state.vd = _VirtualDriveState.from_drive_state(drive_state)
            else:
                if self.__state.vd is None:
                    # Если раньше MSD был отключен
                    self.__state.vd = _VirtualDriveState.from_drive_state(drive_state)

                image = self.__state.vd.image
                if image and (not image.in_storage or not (await image.exists())):
                    # Если только что отключили ручной образ вне хранилища или ранее выбранный образ был удален
                    self.__state.vd.image = None

                self.__state.vd.connected = False

    async def __unsafe_setup_initial(self) -> None:
        if self.__initial_image:
            logger = get_logger(0)
            image = await self.__storage.make_image_by_name(self.__initial_image)
            if (await image.exists()):
                logger.info("Setting up initial image %r ...", self.__initial_image)
                try:
                    self.__drive.set_rw_flag(False)
                    self.__drive.set_cdrom_flag(self.__initial_cdrom)
                    self.__drive.set_image_path(image.path)
                except Exception:
                    logger.exception("Can't setup initial image: ignored")
            else:
                logger.error("Can't find initial image %r: ignored", self.__initial_image)
