# ========================================================================== #
#                                                                            #
#    KVMD - The main PiKVM daemon.                                           #
#                                                                            #
#    Copyright (C) 2018-2023  Maxim Devaev <mdevaev@gmail.com>               #
#                                                                            #
#    This source file is partially based on python-watchdog module.          #
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


import sys
import os
import asyncio
import struct
import dataclasses
import types
import errno

from typing import Generator

from .logging import get_logger

from . import aiotools
from . import libc


# =====
_EVENT_HEAD_FMT = "iIII"
_EVENT_HEAD_SIZE = struct.calcsize(_EVENT_HEAD_FMT)
_EVENTS_BUFFER_LENGTH = 4096 * (_EVENT_HEAD_SIZE + 256)  # count * (head + max_file_name_size + null_character)

_FS_FALLBACK_ENCODING = "utf-8"
_FS_ENCODING = (sys.getfilesystemencoding() or _FS_FALLBACK_ENCODING)


# =====
def _inotify_parsed_buffer(data: bytes) -> Generator[tuple[int, int, int, bytes], None, None]:
    offset = 0
    while offset + _EVENT_HEAD_SIZE <= len(data):
        (wd, mask, cookie, length) = struct.unpack_from("iIII", data, offset)
        name = data[
            offset + _EVENT_HEAD_SIZE  # noqa: E203
            :
            offset + _EVENT_HEAD_SIZE + length
        ].rstrip(b"\0")
        offset += _EVENT_HEAD_SIZE + length
        if wd >= 0:
            yield (wd, mask, cookie, name)


def _inotify_check(retval: int) -> int:
    if retval < 0:
        c_errno = libc.get_errno()
        if c_errno == errno.ENOSPC:  # pylint: disable=no-else-raise
            raise OSError(c_errno, "Inotify watch limit reached")
        elif c_errno == errno.EMFILE:
            raise OSError(c_errno, "Inotify instance limit reached")
        else:
            raise OSError(c_errno, os.strerror(c_errno))
    return retval


def _fs_encode(path: str) -> bytes:
    try:
        return path.encode(_FS_ENCODING, "strict")
    except UnicodeEncodeError:
        return path.encode(_FS_FALLBACK_ENCODING, "strict")


def _fs_decode(path: bytes) -> str:
    try:
        return path.decode(_FS_ENCODING, "strict")
    except UnicodeDecodeError:
        return path.decode(_FS_FALLBACK_ENCODING, "strict")


# =====
class InotifyMask:
    # Userspace events
    ACCESS = 0x00000001         # File was accessed
    ATTRIB = 0x00000004         # Meta-data changed
    CLOSE_WRITE = 0x00000008    # Writable file was closed
    CLOSE_NOWRITE = 0x00000010  # Unwritable file closed
    CREATE = 0x00000100         # Subfile was created
    DELETE = 0x00000200         # Subfile was deleted
    DELETE_SELF = 0x00000400    # Self was deleted
    MODIFY = 0x00000002         # File was modified
    MOVE_SELF = 0x00000800      # Self was moved
    MOVED_FROM = 0x00000040     # File was moved from X
    MOVED_TO = 0x00000080       # File was moved to Y
    OPEN = 0x00000020           # File was opened

    # Events sent by the kernel to a watch
    IGNORED = 0x00008000     # File was ignored
    ISDIR = 0x40000000       # Event occurred against directory
    Q_OVERFLOW = 0x00004000  # Event queued overflowed
    UNMOUNT = 0x00002000     # Backing file system was unmounted

    # Helper userspace events
#    CLOSE = CLOSE_WRITE | CLOSE_NOWRITE  # Close
#    MOVE = MOVED_FROM | MOVED_TO         # Moves

    # Helper for userspace events
#    ALL_EVENTS = (
#        ACCESS
#        | ATTRIB
#        | CLOSE_WRITE
#        | CLOSE_NOWRITE
#        | CREATE
#        | DELETE
#        | DELETE_SELF
#        | MODIFY
#        | MOVE_SELF
#        | MOVED_FROM
#        | MOVED_TO
#        | OPEN
#    )

    # Helper for all modify events
    ALL_MODIFY_EVENTS = (
        CLOSE_WRITE
        | CREATE
        | DELETE
        | DELETE_SELF
        | MODIFY
        | MOVE_SELF
        | MOVED_FROM
        | MOVED_TO
    )

    # Special flags for watch()
#    DONT_FOLLOW = 0x02000000  # Don't follow a symbolic link
#    EXCL_UNLINK = 0x04000000  # Exclude events on unlinked objects
#    MASK_CREATE = 0x10000000  # Don't overwrite existent watchers (since 4.18)
#    MASK_ADD = 0x20000000     # Add to the mask of an existing watch
#    ONESHOT = 0x80000000      # Only send event once
#    ONLYDIR = 0x01000000      # Only watch the path if it's a directory

    @classmethod
    def to_string(cls, mask: int) -> str:
        flags: list[str] = []
        for name in dir(cls):
            if (
                name[0].isupper()
                and not name.startswith("ALL_")
                and name not in ["CLOSE", "MOVE"]
                and mask & getattr(cls, name)
            ):
                flags.append(name)
        return "|".join(flags)


@dataclasses.dataclass(frozen=True, repr=False)
class InotifyEvent:
    wd: int
    mask: int
    cookie: int
    name: str
    path: str

    def __repr__(self) -> str:
        return (
            f"<InotifyEvent: wd={self.wd}, mask={InotifyMask.to_string(self.mask)},"
            f" cookie={self.cookie}, name={self.name}, path={self.path}>"
        )


class Inotify:
    def __init__(self) -> None:
        self.__fd = -1

        self.__wd_by_path: dict[str, int] = {}
        self.__path_by_wd: dict[int, str] = {}

        self.__moved: dict[int, str] = {}

        self.__events_queue: "asyncio.Queue[InotifyEvent]" = asyncio.Queue()

    async def watch(self, mask: int, *paths: str) -> None:
        for path in paths:
            path = os.path.normpath(path)
            assert path not in self.__wd_by_path, path
            get_logger().info("Watching for %s", path)
            # Асинхронно, чтобы не висло на NFS
            wd = _inotify_check(await aiotools.run_async(libc.inotify_add_watch, self.__fd, _fs_encode(path), mask))
            self.__wd_by_path[path] = wd
            self.__path_by_wd[wd] = path

#    def unwatch(self, path: str) -> None:
#        path = os.path.normpath(path)
#        assert path in self.__wd_by_path, path
#        get_logger().info("Unwatching %s", path)
#        wd = self.__wd_by_path[path]
#        _inotify_check(_inotify_rm_watch(self.__fd, wd))
#        del self.__wd_by_path[path]
#        del self.__path_by_wd[wd]

#    def has_events(self) -> bool:
#        return (not self.__events_queue.empty())

    async def get_event(self, timeout: float) -> (InotifyEvent | None):
        assert timeout > 0
        try:
            return (await asyncio.wait_for(
                asyncio.ensure_future(self.__events_queue.get()),
                timeout=timeout,
            ))
        except asyncio.TimeoutError:
            return None

    async def get_series(self, timeout: float) -> list[InotifyEvent]:
        series: list[InotifyEvent] = []
        event = await self.get_event(timeout)
        if event:
            series.append(event)
            while event:
                event = await self.get_event(timeout)
                if event:
                    series.append(event)
        return series

    def __read_and_queue_events(self) -> None:
        logger = get_logger()
        for event in self.__read_parsed_events():
            # XXX: Ни в коем случае не приводить self.__read_parsed_events() к списку.
            # Он использует self.__wd_by_path и self.__path_by_wd, содержимое которых
            # корректируется кодом ниже. В противном случае все сломается.

            if event.mask & InotifyMask.MOVED_FROM:
                self.__moved[event.cookie] = event.path  # Save moved_from_path
            elif event.mask & InotifyMask.MOVED_TO:
                moved_from_path = self.__moved.pop(event.cookie, None)
                if moved_from_path is not None:
                    wd = self.__wd_by_path.pop(moved_from_path, None)
                    if wd is not None:
                        self.__wd_by_path[event.path] = wd
                        self.__path_by_wd[wd] = event.path

            if event.mask & InotifyMask.IGNORED:
                ignored_path = self.__path_by_wd[event.wd]
                if self.__wd_by_path[ignored_path] == event.wd:
                    logger.info("Unwatching %s because IGNORED was received", ignored_path)
                    del self.__wd_by_path[ignored_path]
                continue

            self.__events_queue.put_nowait(event)

    def __read_parsed_events(self) -> Generator[InotifyEvent, None, None]:
        for (wd, mask, cookie, name_bytes) in _inotify_parsed_buffer(self.__read_buffer()):
            wd_path = self.__path_by_wd.get(wd, None)
            if wd_path is not None:
                name = _fs_decode(name_bytes)
                path = (os.path.join(wd_path, name) if name else wd_path)  # Avoid trailing slash
                yield InotifyEvent(wd, mask, cookie, name, path)

    def __read_buffer(self) -> bytes:
        while True:
            try:
                return os.read(self.__fd, _EVENTS_BUFFER_LENGTH)
            except OSError as err:
                if err.errno == errno.EINTR:
                    pass

    def __enter__(self) -> "Inotify":
        assert self.__fd < 0
        self.__fd = _inotify_check(libc.inotify_init())
        asyncio.get_event_loop().add_reader(self.__fd, self.__read_and_queue_events)
        return self

    def __exit__(
        self,
        _exc_type: type[BaseException],
        _exc: BaseException,
        _tb: types.TracebackType,
    ) -> None:

        if self.__fd >= 0:
            asyncio.get_event_loop().remove_reader(self.__fd)
            for wd in list(self.__wd_by_path.values()):
                libc.inotify_rm_watch(self.__fd, wd)
            try:
                os.close(self.__fd)
            except Exception:
                pass
