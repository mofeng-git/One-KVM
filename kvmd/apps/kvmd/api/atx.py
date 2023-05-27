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


from aiohttp.web import Request
from aiohttp.web import Response

from ....htserver import exposed_http
from ....htserver import make_json_response

from ....plugins.atx import BaseAtx

from ....validators.basic import valid_bool
from ....validators.kvm import valid_atx_power_action
from ....validators.kvm import valid_atx_button


# =====
class AtxApi:
    def __init__(self, atx: BaseAtx) -> None:
        self.__atx = atx

    # =====

    @exposed_http("GET", "/atx")
    async def __state_handler(self, _: Request) -> Response:
        return make_json_response(await self.__atx.get_state())

    @exposed_http("POST", "/atx/power")
    async def __power_handler(self, request: Request) -> Response:
        action = valid_atx_power_action(request.query.get("action"))
        wait = valid_bool(request.query.get("wait", False))
        await ({
            "on": self.__atx.power_on,
            "off": self.__atx.power_off,
            "off_hard": self.__atx.power_off_hard,
            "reset_hard": self.__atx.power_reset_hard,
        }[action])(wait)
        return make_json_response()

    @exposed_http("POST", "/atx/click")
    async def __click_handler(self, request: Request) -> Response:
        button = valid_atx_button(request.query.get("button"))
        wait = valid_bool(request.query.get("wait", False))
        await ({
            "power": self.__atx.click_power,
            "power_long": self.__atx.click_power_long,
            "reset": self.__atx.click_reset,
        }[button])(wait)
        return make_json_response()
