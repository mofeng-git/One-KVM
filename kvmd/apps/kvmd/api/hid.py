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


from typing import Dict

import aiohttp.web

from ....plugins.hid import BaseHid

from ....validators.basic import valid_bool

from ....validators.kvm import valid_hid_key
from ....validators.kvm import valid_hid_mouse_move
from ....validators.kvm import valid_hid_mouse_button
from ....validators.kvm import valid_hid_mouse_wheel

from ..http import exposed_http
from ..http import exposed_ws
from ..http import make_json_response


# =====
class HidApi:
    def __init__(self, hid: BaseHid) -> None:
        self.__hid = hid

    # =====

    @exposed_http("GET", "/hid/state")
    async def __state_handler(self, _: aiohttp.web.Request) -> aiohttp.web.Response:
        return make_json_response(self.__hid.get_state())

    @exposed_http("POST", "/hid/reset")
    async def __reset_handler(self, _: aiohttp.web.Request) -> aiohttp.web.Response:
        await self.__hid.reset()
        return make_json_response()

    # =====

    @exposed_ws("key")
    async def __ws_key_handler(self, _: aiohttp.web.WebSocketResponse, event: Dict) -> None:
        try:
            key = valid_hid_key(event["key"])
            state = valid_bool(event["state"])
        except Exception:
            return
        await self.__hid.send_key_event(key, state)

    @exposed_ws("mouse_button")
    async def __ws_mouse_button_handler(self, _: aiohttp.web.WebSocketResponse, event: Dict) -> None:
        try:
            button = valid_hid_mouse_button(event["button"])
            state = valid_bool(event["state"])
        except Exception:
            return
        await self.__hid.send_mouse_button_event(button, state)

    @exposed_ws("mouse_move")
    async def __ws_mouse_move_handler(self, _: aiohttp.web.WebSocketResponse, event: Dict) -> None:
        try:
            to_x = valid_hid_mouse_move(event["to"]["x"])
            to_y = valid_hid_mouse_move(event["to"]["y"])
        except Exception:
            return
        await self.__hid.send_mouse_move_event(to_x, to_y)

    @exposed_ws("mouse_wheel")
    async def __ws_mouse_wheel_handler(self, _: aiohttp.web.WebSocketResponse, event: Dict) -> None:
        try:
            delta_x = valid_hid_mouse_wheel(event["delta"]["x"])
            delta_y = valid_hid_mouse_wheel(event["delta"]["y"])
        except Exception:
            return
        await self.__hid.send_mouse_wheel_event(delta_x, delta_y)
