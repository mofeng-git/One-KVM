# ========================================================================== #
#                                                                            #
#    KVMD - The main PiKVM daemon.                                           #
#                                                                            #
#    Copyright (C) 2018-2022  Maxim Devaev <mdevaev@gmail.com>               #
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


import time

from typing import Dict
from typing import Optional
from typing import Union

import aiohttp

from aiohttp.web import Request
from aiohttp.web import Response
from aiohttp.web import StreamResponse

from ....logging import get_logger

from .... import htclient

from ....htserver import exposed_http
from ....htserver import make_json_response
from ....htserver import make_json_exception
from ....htserver import start_streaming
from ....htserver import stream_json
from ....htserver import stream_json_exception

from ....plugins.msd import BaseMsd

from ....validators.basic import valid_bool
from ....validators.basic import valid_int_f0
from ....validators.basic import valid_float_f01
from ....validators.net import valid_url
from ....validators.kvm import valid_msd_image_name


# ======
class MsdApi:
    def __init__(self, msd: BaseMsd) -> None:
        self.__msd = msd

    # =====

    @exposed_http("GET", "/msd")
    async def __state_handler(self, _: Request) -> Response:
        return make_json_response(await self.__msd.get_state())

    @exposed_http("POST", "/msd/set_params")
    async def __set_params_handler(self, request: Request) -> Response:
        params = {
            key: validator(request.query.get(param))
            for (param, key, validator) in [
                ("image", "name", (lambda arg: str(arg).strip() and valid_msd_image_name(arg))),
                ("cdrom", "cdrom", valid_bool),
                ("rw", "rw", valid_bool),
            ]
            if request.query.get(param) is not None
        }
        await self.__msd.set_params(**params)  # type: ignore
        return make_json_response()

    @exposed_http("POST", "/msd/set_connected")
    async def __set_connected_handler(self, request: Request) -> Response:
        await self.__msd.set_connected(valid_bool(request.query.get("connected")))
        return make_json_response()

    # =====

    @exposed_http("GET", "/msd/read")
    async def __read_handler(self, request: Request) -> StreamResponse:
        name = valid_msd_image_name(request.query.get("image"))
        async with self.__msd.read_image(name) as reader:
            size = reader.get_total_size()
            response = await start_streaming(request, "application/octet-stream", size, name)
            async for chunk in reader.read_chunked():
                await response.write(chunk)
            return response

    @exposed_http("POST", "/msd/write")
    async def __write_handler(self, request: Request) -> Response:
        name = valid_msd_image_name(request.query.get("image"))
        size = valid_int_f0(request.content_length)
        remove_incomplete = self.__get_remove_incomplete(request)
        written = 0
        async with self.__msd.write_image(name, size, remove_incomplete) as writer:
            chunk_size = writer.get_chunk_size()
            while True:
                chunk = await request.content.read(chunk_size)
                if not chunk:
                    break
                written = await writer.write_chunk(chunk)
        return make_json_response(self.__make_write_info(name, size, written))

    @exposed_http("POST", "/msd/write_remote")
    async def __write_remote_handler(self, request: Request) -> Union[Response, StreamResponse]:  # pylint: disable=too-many-locals
        url = valid_url(request.query.get("url"))
        insecure = valid_bool(request.query.get("insecure", "0"))
        timeout = valid_float_f01(request.query.get("timeout", 10.0))
        remove_incomplete = self.__get_remove_incomplete(request)

        name = ""
        size = written = 0
        response: Optional[StreamResponse] = None

        async def stream_write_info() -> None:
            assert response is not None
            await stream_json(response, self.__make_write_info(name, size, written))

        try:
            async with htclient.download(
                url=url,
                verify=(not insecure),
                timeout=timeout,
                read_timeout=(7 * 24 * 3600),
            ) as remote:

                name = str(request.query.get("image", "")).strip()
                if len(name) == 0:
                    name = htclient.get_filename(remote)
                name = valid_msd_image_name(name)

                size = valid_int_f0(remote.content_length)

                get_logger(0).info("Downloading image %r as %r to MSD ...", url, name)
                async with self.__msd.write_image(name, size, remove_incomplete) as writer:
                    chunk_size = writer.get_chunk_size()
                    response = await start_streaming(request, "application/x-ndjson")
                    await stream_write_info()
                    last_report_ts = 0
                    async for chunk in remote.content.iter_chunked(chunk_size):
                        written = await writer.write_chunk(chunk)
                        now = int(time.time())
                        if last_report_ts + 1 < now:
                            await stream_write_info()
                            last_report_ts = now

                await stream_write_info()
                return response

        except Exception as err:
            if response is not None:
                await stream_write_info()
                await stream_json_exception(response, err)
            elif isinstance(err, aiohttp.ClientError):
                return make_json_exception(err, 400)
            raise

    def __get_remove_incomplete(self, request: Request) -> Optional[bool]:
        flag: Optional[str] = request.query.get("remove_incomplete")
        return (valid_bool(flag) if flag is not None else None)

    def __make_write_info(self, name: str, size: int, written: int) -> Dict:
        return {"image": {"name": name, "size": size, "written": written}}

    # =====

    @exposed_http("POST", "/msd/remove")
    async def __remove_handler(self, request: Request) -> Response:
        await self.__msd.remove(valid_msd_image_name(request.query.get("image")))
        return make_json_response()

    @exposed_http("POST", "/msd/reset")
    async def __reset_handler(self, _: Request) -> Response:
        await self.__msd.reset()
        return make_json_response()
