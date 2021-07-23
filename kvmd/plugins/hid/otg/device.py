# ========================================================================== #
#                                                                            #
#    KVMD - The main Pi-KVM daemon.                                          #
#                                                                            #
#    Copyright (C) 2018-2021  Maxim Devaev <mdevaev@gmail.com>               #
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
import select
import multiprocessing
import queue
import errno
import time

from typing import Dict

from ....logging import get_logger

from .... import tools
from .... import aiomulti
from .... import aioproc

from .usb import UsbDeviceController
from .events import BaseEvent


# =====
class BaseDeviceProcess(multiprocessing.Process):  # pylint: disable=too-many-instance-attributes
    def __init__(  # pylint: disable=too-many-arguments
        self,
        name: str,
        read_size: int,
        initial_state: Dict,
        notifier: aiomulti.AioProcessNotifier,

        udc: UsbDeviceController,

        device_path: str,
        select_timeout: float,
        write_retries: int,
        write_retries_delay: float,
        reopen_delay: float,
        noop: bool,
    ) -> None:

        super().__init__(daemon=True)

        self.__name = name
        self.__read_size = read_size

        self.__udc = udc

        self.__device_path = device_path
        self.__select_timeout = select_timeout
        self.__write_retries = write_retries
        self.__write_retries_delay = write_retries_delay
        self.__reopen_delay = reopen_delay
        self.__noop = noop

        self.__fd = -1
        self.__events_queue: "multiprocessing.Queue[BaseEvent]" = multiprocessing.Queue()
        self.__state_flags = aiomulti.AioSharedFlags({"online": True, **initial_state}, notifier)
        self.__stop_event = multiprocessing.Event()

    def run(self) -> None:
        logger = aioproc.settle(f"HID-{self.__name}", f"hid-{self.__name}")
        while not self.__stop_event.is_set():
            try:
                while not self.__stop_event.is_set():
                    if self.__ensure_device():  # Check device and process reports if needed
                        self.__read_all_reports()
                    try:
                        event = self.__events_queue.get(timeout=0.1)
                    except queue.Empty:
                        if not self.__udc.can_operate():
                            self._clear_queue()
                            self.__close_device()
                    else:
                        if not self._process_event(event):
                            self._clear_queue()
            except Exception:
                logger.exception("Unexpected HID-%s error", self.__name)
                self._clear_queue()
                self.__close_device()
                time.sleep(1)

        self.__close_device()

    async def get_state(self) -> Dict:
        return (await self.__state_flags.get())

    # =====

    def _process_event(self, event: BaseEvent) -> bool:
        raise NotImplementedError

    def _process_read_report(self, report: bytes) -> None:
        pass

    def _update_state(self, **kwargs: bool) -> None:
        assert "online" not in kwargs
        self.__state_flags.update(**kwargs)

    # =====

    def _stop(self) -> None:
        if self.is_alive():
            get_logger().info("Stopping HID-%s daemon ...", self.__name)
            self.__stop_event.set()
        if self.is_alive() or self.exitcode is not None:
            self.join()

    def _queue_event(self, event: BaseEvent) -> None:
        self.__events_queue.put_nowait(event)

    def _clear_queue(self) -> None:
        tools.clear_queue(self.__events_queue)

    def _ensure_write(self, report: bytes, reopen: bool=False, close: bool=False) -> bool:
        if reopen:
            self.__close_device()
        try:
            if self.__ensure_device():
                return self.__write_report(report)
            return False
        finally:
            if close:
                self.__close_device()

    # =====

    def __write_report(self, report: bytes) -> bool:
        if self.__noop:
            return True

        assert self.__fd >= 0
        logger = get_logger()

        retries = self.__write_retries
        while retries:
            try:
                written = os.write(self.__fd, report)
                if written == len(report):
                    self.__state_flags.update(online=True)
                    return True
                else:
                    logger.error("HID-%s write() error: written (%s) != report length (%d)",
                                 self.__name, written, len(report))
            except Exception as err:
                if isinstance(err, OSError) and (
                    # https://github.com/raspberrypi/linux/commit/61b7f805dc2fd364e0df682de89227e94ce88e25
                    err.errno == errno.EAGAIN  # pylint: disable=no-member
                    or err.errno == errno.ESHUTDOWN  # pylint: disable=no-member
                ):
                    logger.debug("HID-%s busy/unplugged (write): %s", self.__name, tools.efmt(err))
                else:
                    logger.exception("Can't write report to HID-%s", self.__name)

            retries -= 1

            if retries:
                logger.debug("HID-%s write retries left: %d", self.__name, retries)
                time.sleep(self.__write_retries_delay)

        self.__close_device()
        return False

    def __read_all_reports(self) -> None:
        if self.__noop or self.__read_size == 0:
            return

        assert self.__fd >= 0
        logger = get_logger()

        read = True
        while read:
            try:
                read = bool(select.select([self.__fd], [], [], 0)[0])
            except Exception as err:
                logger.error("Can't select() for read HID-%s: %s", self.__name, tools.efmt(err))
                break

            if read:
                try:
                    report = os.read(self.__fd, self.__read_size)
                except Exception as err:
                    if isinstance(err, OSError) and err.errno == errno.EAGAIN:  # pylint: disable=no-member
                        logger.debug("HID-%s busy/unplugged (read): %s", self.__name, tools.efmt(err))
                    else:
                        logger.exception("Can't read report from HID-%s", self.__name)
                else:
                    self._process_read_report(report)

    def __ensure_device(self) -> bool:
        if self.__noop:
            return True

        logger = get_logger()

        if self.__fd < 0:
            if self.__udc.can_operate():
                try:
                    flags = os.O_NONBLOCK
                    flags |= (os.O_RDWR if self.__read_size else os.O_WRONLY)
                    self.__fd = os.open(self.__device_path, flags)
                except Exception as err:
                    logger.error("Can't open HID-%s device %s: %s", self.__name, self.__device_path, tools.efmt(err))
                    time.sleep(self.__reopen_delay)
            else:
                time.sleep(self.__reopen_delay)

        if self.__fd >= 0:
            try:
                if select.select([], [self.__fd], [], self.__select_timeout)[1]:
                    self.__state_flags.update(online=True)
                    return True
                else:
                    logger.debug("HID-%s is busy/unplugged (write select)", self.__name)
            except Exception as err:
                logger.error("Can't select() for write HID-%s: %s", self.__name, tools.efmt(err))
            self.__close_device()

        self.__state_flags.update(online=False)
        return False

    def __close_device(self) -> None:
        if self.__fd >= 0:
            try:
                os.close(self.__fd)
            except Exception:
                pass
            finally:
                self.__fd = -1
