# ========================================================================== #
#                                                                            #
#    KVMD - The main PiKVM daemon.                                           #
#                                                                            #
#    Copyright (C) 2018-2023  Maxim Devaev <mdevaev@gmail.com>               #
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


from typing import Any

from . import raise_error
from . import check_string_in_list

from .basic import valid_stripped_string_not_empty
from .basic import valid_number
from .basic import valid_string_list
from .os import valid_printable_filename


# =====
def valid_atx_power_action(arg: Any) -> str:
    return check_string_in_list(arg, "ATX power action", ["on", "off", "off_hard", "reset_hard"])


def valid_atx_button(arg: Any) -> str:
    return check_string_in_list(arg, "ATX button", ["power", "power_long", "reset"])


def valid_msd_image_name(arg: Any) -> str:
    name = "MSD image name"
    arg = valid_stripped_string_not_empty(arg, name)
    parts: list[str] = list(filter(None, arg.split("/")))
    if len(parts) == 0:
        raise_error(arg, name)
    for (index, part) in enumerate(list(parts)):
        parts[index] = valid_printable_filename(part, name=name)
    return "/".join(parts)


def valid_info_fields(arg: Any, variants: set[str]) -> set[str]:
    return set(valid_string_list(
        arg=str(arg).strip(),
        subval=(lambda field: check_string_in_list(field, "info field", variants)),
        name="info fields list",
    ))


def valid_log_seek(arg: Any) -> int:
    return int(valid_number(arg, min=0, name="log seek"))


def valid_stream_quality(arg: Any) -> int:
    return int(valid_number(arg, min=1, max=100, name="stream quality"))


def valid_stream_fps(arg: Any) -> int:
    return int(valid_number(arg, min=0, max=120, name="stream FPS"))


def valid_stream_resolution(arg: Any) -> str:
    name = "stream resolution"
    arg = valid_stripped_string_not_empty(arg, name)
    parts = arg.split("x")
    if len(parts) != 2:
        raise_error(arg, name)
    width = int(valid_number(parts[0], min=1, name=f"{name} (width)"))
    height = int(valid_number(parts[1], min=1, name=f"{name} (height)"))
    return f"{width}x{height}"


def valid_stream_h264_bitrate(arg: Any) -> int:
    return int(valid_number(arg, min=25, max=20000, name="stream H264 bitrate"))


def valid_stream_h264_gop(arg: Any) -> int:
    return int(valid_number(arg, min=0, max=60, name="stream H264 GOP"))
