# ========================================================================== #
#                                                                            #
#    KVMD - The main PiKVM daemon.                                           #
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
import stat
import fcntl
import struct
import dataclasses

from typing import IO
from typing import Optional

from .... import aiotools
from .... import aiofs

from .. import MsdImageWriter


# =====
_IMAGE_INFO_SIZE = 4096
_IMAGE_INFO_MAGIC_SIZE = 16
_IMAGE_INFO_NAME_SIZE = 256
_IMAGE_INFO_PADS_SIZE = _IMAGE_INFO_SIZE - _IMAGE_INFO_NAME_SIZE - 1 - 8 - _IMAGE_INFO_MAGIC_SIZE * 8
_IMAGE_INFO_FORMAT = ">%dL%dc?Q%dx%dL" % (
    _IMAGE_INFO_MAGIC_SIZE,
    _IMAGE_INFO_NAME_SIZE,
    _IMAGE_INFO_PADS_SIZE,
    _IMAGE_INFO_MAGIC_SIZE,
)
_IMAGE_INFO_MAGIC = [0x1ACE1ACE] * _IMAGE_INFO_MAGIC_SIZE


# =====
@dataclasses.dataclass(frozen=True)
class ImageInfo:
    name: str
    size: int
    complete: bool

    @classmethod
    def from_bytes(cls, data: bytes) -> Optional["ImageInfo"]:
        try:
            parsed = list(struct.unpack(_IMAGE_INFO_FORMAT, data))
        except struct.error:
            pass
        else:
            magic_begin = parsed[:_IMAGE_INFO_MAGIC_SIZE]
            magic_end = parsed[-_IMAGE_INFO_MAGIC_SIZE:]
            if magic_begin == magic_end == _IMAGE_INFO_MAGIC:
                image_name_bytes = b"".join(parsed[
                    _IMAGE_INFO_MAGIC_SIZE  # noqa: E203
                    :
                    _IMAGE_INFO_MAGIC_SIZE + _IMAGE_INFO_NAME_SIZE
                ])
                return ImageInfo(
                    name=image_name_bytes.decode("utf-8", errors="ignore").strip("\x00").strip(),
                    size=parsed[_IMAGE_INFO_MAGIC_SIZE + _IMAGE_INFO_NAME_SIZE + 1],
                    complete=parsed[_IMAGE_INFO_MAGIC_SIZE + _IMAGE_INFO_NAME_SIZE],
                )
        return None

    def to_bytes(self) -> bytes:
        return struct.pack(
            _IMAGE_INFO_FORMAT,
            *_IMAGE_INFO_MAGIC,
            *memoryview((  # type: ignore
                self.name.encode("utf-8")
                + b"\x00" * _IMAGE_INFO_NAME_SIZE
            )[:_IMAGE_INFO_NAME_SIZE]).cast("c"),
            self.complete,
            self.size,
            *_IMAGE_INFO_MAGIC,
        )


@dataclasses.dataclass(frozen=True)
class DeviceInfo:
    path: str
    size: int
    free: int
    image: Optional[ImageInfo]

    @classmethod
    async def read(cls, device_path: str) -> "DeviceInfo":
        return (await aiotools.run_async(cls.__inner_read, device_path))

    @classmethod
    def __inner_read(cls, device_path: str) -> "DeviceInfo":
        if not stat.S_ISBLK(os.stat(device_path).st_mode):
            raise RuntimeError(f"Not a block device: {device_path}")

        with open(device_path, "rb") as device_file:
            # size = BLKGETSIZE * BLKSSZGET
            size = _ioctl_uint32(device_file, 0x1260) * _ioctl_uint32(device_file, 0x1268)
            device_file.seek(size - _IMAGE_INFO_SIZE)
            image_info = ImageInfo.from_bytes(device_file.read())

        return DeviceInfo(
            path=device_path,
            size=size,
            free=(size - image_info.size if image_info else size),
            image=image_info,
        )

    async def write_image_info(self, device_writer: MsdImageWriter, complete: bool) -> bool:
        device_file = device_writer.get_file()
        state = device_writer.get_state()
        image_info = ImageInfo(state["name"], state["written"], complete)

        if self.size - image_info.size > _IMAGE_INFO_SIZE:
            await device_file.seek(self.size - _IMAGE_INFO_SIZE)  # type: ignore
            await device_file.write(image_info.to_bytes())  # type: ignore
            await aiofs.afile_sync(device_file)
            await device_file.seek(0)  # type: ignore
            return True
        return False  # Device is full


def _ioctl_uint32(device_file: IO, request: int) -> int:
    buf = b"\0" * 4
    buf = fcntl.ioctl(device_file.fileno(), request, buf)  # type: ignore
    result = struct.unpack("I", buf)[0]
    assert result > 0, (device_file, request, buf)
    return result
