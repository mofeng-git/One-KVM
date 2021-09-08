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


import io
import functools

from aiohttp.web import Request
from aiohttp.web import Response

from PIL import Image

from ....validators.basic import valid_bool
from ....validators.basic import valid_int_f0
from ....validators.kvm import valid_stream_quality

from .... import aiotools

from ..http import UnavailableError
from ..http import exposed_http
from ..http import make_json_response

from ..streamer import StreamerSnapshot
from ..streamer import Streamer


# =====
class StreamerApi:
    def __init__(self, streamer: Streamer) -> None:
        self.__streamer = streamer

    # =====

    @exposed_http("GET", "/streamer")
    async def __state_handler(self, _: Request) -> Response:
        return make_json_response(await self.__streamer.get_state())

    @exposed_http("GET", "/streamer/snapshot")
    async def __take_snapshot_handler(self, request: Request) -> Response:
        snapshot = await self.__streamer.take_snapshot(
            save=valid_bool(request.query.get("save", "false")),
            load=valid_bool(request.query.get("load", "false")),
            allow_offline=valid_bool(request.query.get("allow_offline", "false")),
        )
        if snapshot:
            if valid_bool(request.query.get("preview", "false")):
                data = await self.__make_preview(
                    snapshot=snapshot,
                    max_width=valid_int_f0(request.query.get("preview_max_width", "0")),
                    max_height=valid_int_f0(request.query.get("preview_max_height", "0")),
                    quality=valid_stream_quality(request.query.get("preview_quality", "80")),
                )
            else:
                data = snapshot.data
            return Response(
                body=data,
                headers=dict(snapshot.headers),
                content_type="image/jpeg",
            )
        raise UnavailableError()

    @exposed_http("DELETE", "/streamer/snapshot")
    async def __remove_snapshot_handler(self, _: Request) -> Response:
        self.__streamer.remove_snapshot()
        return make_json_response()

    # =====

    async def __make_preview(self, snapshot: StreamerSnapshot, max_width: int, max_height: int, quality: int) -> bytes:
        if max_width == 0 and max_height == 0:
            max_width = snapshot.width // 5
            max_height = snapshot.height // 5
        else:
            max_width = min((max_width or snapshot.width), snapshot.width)
            max_height = min((max_height or snapshot.height), snapshot.height)

        if max_width == snapshot.width and max_height == snapshot.height:
            return snapshot.data
        else:
            return (await aiotools.run_async(self.__inner_make_preview, snapshot, max_width, max_height, quality))

    @functools.lru_cache(maxsize=1)
    def __inner_make_preview(self, snapshot: StreamerSnapshot, max_width: int, max_height: int, quality: int) -> bytes:
        assert 0 < max_width <= snapshot.width
        assert 0 < max_height <= snapshot.height
        assert not (max_width == snapshot.width and max_height == snapshot.height)
        with io.BytesIO(snapshot.data) as snapshot_bio:
            with io.BytesIO() as preview_bio:
                with Image.open(snapshot_bio) as image:
                    image.thumbnail((max_width, max_height), Image.ANTIALIAS)
                    image.save(preview_bio, format="jpeg", quality=quality)
                    return preview_bio.getvalue()
