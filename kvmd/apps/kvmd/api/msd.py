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


from aiohttp.web import Request
from aiohttp.web import Response

from ....logging import get_logger

from ....plugins.msd import BaseMsd

from ....validators.basic import valid_bool

from ....validators.kvm import valid_msd_image_name

from ..http import exposed_http
from ..http import make_json_response
from ..http import get_multipart_field


# ======
class MsdApi:
    def __init__(self, msd: BaseMsd, sync_chunk_size: int) -> None:
        self.__msd = msd
        self.__sync_chunk_size = sync_chunk_size

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
            ]
            if request.query.get(param) is not None
        }
        await self.__msd.set_params(**params)  # type: ignore
        return make_json_response()

    @exposed_http("POST", "/msd/connect")
    async def __connect_handler(self, _: Request) -> Response:
        await self.__msd.connect()
        return make_json_response()

    @exposed_http("POST", "/msd/disconnect")
    async def __disconnect_handler(self, _: Request) -> Response:
        await self.__msd.disconnect()
        return make_json_response()

    @exposed_http("POST", "/msd/write")
    async def __write_handler(self, request: Request) -> Response:
        logger = get_logger(0)
        reader = await request.multipart()
        name = ""
        written = 0
        try:
            name_field = await get_multipart_field(reader, "image")
            name = valid_msd_image_name((await name_field.read()).decode("utf-8"))

            data_field = await get_multipart_field(reader, "data")

            async with self.__msd.write_image(name):
                logger.info("Writing image %r to MSD ...", name)
                while True:
                    chunk = await data_field.read_chunk(self.__sync_chunk_size)
                    if not chunk:
                        break
                    written = await self.__msd.write_image_chunk(chunk)
        finally:
            if written != 0:
                logger.info("Written image %r with size=%d bytes to MSD", name, written)
        return make_json_response({"image": {"name": name, "size": written}})

    @exposed_http("POST", "/msd/remove")
    async def __remove_handler(self, request: Request) -> Response:
        await self.__msd.remove(valid_msd_image_name(request.query.get("image")))
        return make_json_response()

    @exposed_http("POST", "/msd/reset")
    async def __reset_handler(self, _: Request) -> Response:
        await self.__msd.reset()
        return make_json_response()
