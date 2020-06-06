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

from ..wol import WakeOnLan

from ..http import exposed_http
from ..http import make_json_response


# =====
class WolApi:
    def __init__(self, wol: WakeOnLan) -> None:
        self.__wol = wol

    # =====

    @exposed_http("GET", "/wol")
    async def __state_handler(self, _: Request) -> Response:
        return make_json_response(await self.__wol.get_state())

    @exposed_http("POST", "/wol/wakeup")
    async def __wakeup_handler(self, _: Request) -> Response:
        await self.__wol.wakeup()
        return make_json_response()
