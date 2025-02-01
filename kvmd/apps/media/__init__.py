# ========================================================================== #
#                                                                            #
#    KVMD - The main PiKVM daemon.                                           #
#                                                                            #
#    Copyright (C) 2020  Maxim Devaev <mdevaev@gmail.com>                    #
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


from ...clients.streamer import StreamerFormats
from ...clients.streamer import MemsinkStreamerClient

from .. import init

from .server import MediaServer


# =====
def main(argv: (list[str] | None)=None) -> None:
    config = init(
        prog="kvmd-media",
        description="The media proxy",
        check_run=True,
        argv=argv,
    )[2].media

    def make_streamer(name: str, fmt: int) -> (MemsinkStreamerClient | None):
        if getattr(config.memsink, name).sink:
            return MemsinkStreamerClient(name.upper(), fmt, **getattr(config.memsink, name)._unpack())
        return None

    MediaServer(
        h264_streamer=make_streamer("h264", StreamerFormats.H264),
        jpeg_streamer=make_streamer("jpeg", StreamerFormats.JPEG),
    ).run(**config.server._unpack())
