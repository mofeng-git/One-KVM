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
import select
import signal
import multiprocessing
import multiprocessing.queues
import queue
import errno
import time

from typing import Dict
from typing import Any

import setproctitle

from ....logging import get_logger


# =====
class BaseEvent:
    pass


class BaseDeviceProcess(multiprocessing.Process):  # pylint: disable=too-many-instance-attributes
    def __init__(
        self,
        name: str,
        read_size: int,
        initial_state: Dict,
        changes_queue: multiprocessing.queues.Queue,

        device_path: str,
        select_timeout: float,
        write_retries: int,
        write_retries_delay: float,
        noop: bool,
    ) -> None:

        super().__init__(daemon=True)

        self.__name = name
        self.__read_size = read_size
        self.__changes_queue = changes_queue

        self.__device_path = device_path
        self.__select_timeout = select_timeout
        self.__write_retries = write_retries
        self.__write_retries_delay = write_retries_delay
        self.__noop = noop

        self.__fd = -1
        self.__events_queue: multiprocessing.queues.Queue = multiprocessing.Queue()
        self.__state_shared = multiprocessing.Manager().dict(online=True, **initial_state)  # type: ignore
        self.__stop_event = multiprocessing.Event()

    def run(self) -> None:
        logger = get_logger(0)

        logger.info("Started HID-%s pid=%d", self.__name, os.getpid())
        signal.signal(signal.SIGINT, signal.SIG_IGN)
        setproctitle.setproctitle(f"kvmd/hid-{self.__name}: {setproctitle.getproctitle()}")

        while not self.__stop_event.is_set():
            try:
                while not self.__stop_event.is_set():
                    if self.__ensure_device():  # Check device and process reports if needed
                        self.__read_all_reports()
                    try:
                        event: BaseEvent = self.__events_queue.get(timeout=0.1)
                    except queue.Empty:
                        pass
                    else:
                        self._process_event(event)
            except Exception:
                logger.exception("Unexpected HID-%s error", self.__name)
                self.__close_device()
            finally:
                time.sleep(1)

        self.__close_device()

    def get_state(self) -> Dict:
        return dict(self.__state_shared)

    # =====

    def _process_event(self, event: BaseEvent) -> None:
        raise NotImplementedError

    def _process_read_report(self, report: bytes) -> None:
        pass

    # =====

    def _stop(self) -> None:
        if self.is_alive():
            get_logger().info("Stopping HID-%s daemon ...", self.__name)
            self.__stop_event.set()
        if self.exitcode is not None:
            self.join()

    def _queue_event(self, event: BaseEvent) -> None:
        self.__events_queue.put(event)

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

    def _update_state(self, key: str, value: Any) -> None:
        if self.__state_shared[key] != value:
            self.__state_shared[key] = value
            self.__changes_queue.put(None)

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
                    self._update_state("online", True)
                    return True
                else:
                    logger.error("HID-%s write() error: written (%s) != report length (%d)",
                                 self.__name, written, len(report))
            except Exception as err:
                if isinstance(err, OSError) and err.errno == errno.EAGAIN:  # pylint: disable=no-member
                    logger.debug("HID-%s busy/unplugged (write): %s: %s",
                                 self.__name, type(err).__name__, err)
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
                logger.error("Can't select() for read HID-%s: %s: %s", self.__name, type(err).__name__, err)
                break

            if read:
                try:
                    report = os.read(self.__fd, self.__read_size)
                except Exception as err:
                    if isinstance(err, OSError) and err.errno == errno.EAGAIN:  # pylint: disable=no-member
                        logger.debug("HID-%s busy/unplugged (read): %s: %s",
                                     self.__name, type(err).__name__, err)
                    else:
                        logger.exception("Can't read report from HID-%s", self.__name)
                else:
                    self._process_read_report(report)

    def __ensure_device(self) -> bool:
        if self.__noop:
            return True

        logger = get_logger()

        if self.__fd < 0:
            try:
                flags = os.O_NONBLOCK
                flags |= (os.O_RDWR if self.__read_size else os.O_WRONLY)
                self.__fd = os.open(self.__device_path, flags)
            except FileNotFoundError:
                logger.error("Missing HID-%s device: %s", self.__name, self.__device_path)
                time.sleep(self.__select_timeout)
            except Exception as err:
                logger.error("Can't open HID-%s device: %s: %s: %s",
                             self.__name, self.__device_path, type(err).__name__, err)
                time.sleep(self.__select_timeout)

        if self.__fd >= 0:
            try:
                if select.select([], [self.__fd], [], self.__select_timeout)[1]:
                    self._update_state("online", True)
                    return True
                else:
                    logger.debug("HID-%s is busy/unplugged (write select)", self.__name)
            except Exception as err:
                logger.error("Can't select() for write HID-%s: %s: %s", self.__name, type(err).__name__, err)
            self.__close_device()

        self._update_state("online", False)
        return False

    def __close_device(self) -> None:
        if self.__fd >= 0:
            try:
                os.close(self.__fd)
            except Exception:
                pass
            finally:
                self.__fd = -1
