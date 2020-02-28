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
import multiprocessing
import multiprocessing.queues
import multiprocessing.sharedctypes
import queue

from typing import Dict

from . import aiotools


# =====
class AioProcessNotifier:
    def __init__(self) -> None:
        self.__queue: multiprocessing.queues.Queue = multiprocessing.Queue()
        self.__pid = os.getpid()

    def notify(self) -> None:
        assert os.getpid() != self.__pid, "Child only"
        self.__queue.put(None)

    async def wait(self) -> None:
        assert os.getpid() == self.__pid, "Parent only"
        while not (await aiotools.run_async(self.__inner_wait)):
            pass

    def __inner_wait(self) -> bool:
        try:
            self.__queue.get(timeout=0.1)
            while not self.__queue.empty():
                self.__queue.get()
            return True
        except queue.Empty:
            return False


class AioSharedFlags:
    def __init__(
        self,
        initial: Dict[str, bool],
        notifier: AioProcessNotifier,
    ) -> None:

        self.__local_flags = dict(initial)  # To fast comparsion
        self.__notifier = notifier

        self.__shared_flags: Dict[str, multiprocessing.sharedctypes.RawValue] = {
            key: multiprocessing.RawValue("i", int(value))  # type: ignore
            for (key, value) in initial.items()
        }
        self.__lock = multiprocessing.Lock()
        self.__pid = os.getpid()

    def update(self, **kwargs: bool) -> None:
        assert os.getpid() != self.__pid, "Child only"
        changed = False
        try:
            for (key, value) in kwargs.items():
                value = bool(value)
                if self.__local_flags[key] != value:
                    if not changed:
                        self.__lock.acquire()
                    self.__shared_flags[key].value = int(value)
                    self.__local_flags[key] = value
                    changed = True
        finally:
            if changed:
                self.__lock.release()
                self.__notifier.notify()

    async def get(self) -> Dict[str, bool]:
        return (await aiotools.run_async(self.__inner_get))

    def __inner_get(self) -> Dict[str, bool]:
        assert os.getpid() == self.__pid, "Parent only"
        with self.__lock:
            return {
                key: bool(shared.value)
                for (key, shared) in self.__shared_flags.items()
            }
