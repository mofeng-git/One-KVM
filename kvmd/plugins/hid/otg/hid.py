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

import setproctitle

from ....logging import get_logger


# =====
class BaseEvent:
    pass


class DeviceProcess(multiprocessing.Process):  # pylint: disable=too-many-instance-attributes
    def __init__(
        self,
        name: str,
        device_path: str,
        timeout: float,
        retries: int,
        retries_delay: float,
        noop: bool,
    ) -> None:

        super().__init__(daemon=True)

        self.__name = name

        self.__device_path = device_path
        self.__timeout = timeout
        self.__retries = retries
        self.__retries_delay = retries_delay
        self.__noop = noop

        self.__fd = -1
        self.__events_queue: multiprocessing.queues.Queue = multiprocessing.Queue()
        self.__online_shared = multiprocessing.Value("i", 1)
        self.__stop_event = multiprocessing.Event()

    def run(self) -> None:
        logger = get_logger(0)

        logger.info("Started HID-%s pid=%d", self.__name, os.getpid())
        signal.signal(signal.SIGINT, signal.SIG_IGN)
        setproctitle.setproctitle(f"[hid-{self.__name}] {setproctitle.getproctitle()}")

        while not self.__stop_event.is_set():
            try:
                while not self.__stop_event.is_set():
                    passed = 0
                    try:
                        event: BaseEvent = self.__events_queue.get(timeout=0.05)
                    except queue.Empty:
                        if passed >= 20:  # 20 * 0.05 = 1 sec
                            self._ensure_device()  # Check device
                            passed = 0
                        else:
                            passed += 1
                    else:
                        self._process_event(event)
                        passed = 0
            except Exception:
                logger.error("Unexpected HID-%s error", self.__name)
                self._close_device()
            finally:
                time.sleep(1)

        self._close_device()

    def is_online(self) -> bool:
        return bool(self.__online_shared.value)

    def _stop(self) -> None:
        if self.is_alive():
            get_logger().info("Stopping HID-%s daemon ...", self.__name)
            self.__stop_event.set()
        if self.exitcode is not None:
            self.join()

    def _process_event(self, event: BaseEvent) -> None:
        raise NotImplementedError

    def _queue_event(self, event: BaseEvent) -> None:
        self.__events_queue.put(event)

    def _write_report(self, report: bytes) -> bool:
        if self.__noop:
            return True

        assert self.__fd >= 0
        logger = get_logger()

        retries = self.__retries
        while retries:
            try:
                written = os.write(self.__fd, report)
                if written == len(report):
                    self.__online_shared.value = 1
                    return True
                else:
                    logger.error("HID-%s write error: written (%s) != report length (%d)",
                                 self.__name, written, len(report))
                    self._close_device()
            except Exception as err:
                if isinstance(err, OSError) and errno == errno.EAGAIN:
                    msg = "Can't write report to HID-%s {}: %s: %s"
                    msg.format(" (maybe unplugged)" if retries == 1 else "")
                    logger.error(msg, self.__name, type(err).__name__, err)  # TODO: debug
                else:
                    logger.exception("Can't write report to HID-%s", self.__name)
                    self._close_device()

            retries -= 1
            self.__online_shared.value = 0

            if retries:
                logger.error("Retries left (HID-%s, write_report): %d", self.__name, retries)
                time.sleep(self.__retries_delay)

        return False

    def _ensure_device(self) -> bool:
        if self.__noop:
            return True

        logger = get_logger()

        if self.__fd < 0:
            try:
                self.__fd = os.open(self.__device_path, os.O_WRONLY|os.O_NONBLOCK)
            except FileNotFoundError:
                logger.error("Missing HID-%s device: %s", self.__name, self.__device_path)
            except Exception:
                logger.exception("Can't open HID-%s device: %s", self.__name, self.__device_path)

        if self.__fd >= 0:
            retries = self.__retries
            while retries:
                try:
                    if select.select([], [self.__fd], [], self.__timeout)[1]:
                        self.__online_shared.value = 1
                        return True
                    else:
                        msg = "HID-%s is unavailable for writing"
                        if retries == 1:
                            msg += " (maybe unplugged)"
                        logger.error(msg, self.__name)  # TODO: debug
                except Exception as err:
                    logger.error("Can't select() HID-%s: %s: %s", self.__name, type(err).__name__, err)

                retries -= 1
                self.__online_shared.value = 0

                if retries:
                    logger.error("Retries left (HID-%s, ensure_device): %d", self.__name, retries)
                    time.sleep(self.__retries_delay)

            self._close_device()

        return False

    def _close_device(self) -> None:
        if self.__fd >= 0:
            try:
                os.close(self.__fd)
            except Exception:
                pass
            finally:
                self.__fd = -1
