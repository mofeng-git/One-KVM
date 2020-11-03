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


import os
import stat
import functools

from typing import Dict
from typing import Set

from aiohttp.web import Request
from aiohttp.web import Response
from aiohttp.web import WebSocketResponse

from ....plugins.hid import BaseHid

from ....validators import raise_error
from ....validators.basic import valid_bool
from ....validators.basic import valid_int_f0
from ....validators.os import valid_printable_filename
from ....validators.kvm import valid_hid_key
from ....validators.kvm import valid_hid_mouse_move
from ....validators.kvm import valid_hid_mouse_button
from ....validators.kvm import valid_hid_mouse_delta

from ....keyboard.keysym import build_symmap
from ....keyboard.printer import text_to_web_keys

from ..http import exposed_http
from ..http import exposed_ws
from ..http import make_json_response


# =====
class HidApi:
    def __init__(self, hid: BaseHid, keymap_path: str) -> None:
        self.__hid = hid

        self.__keymaps_dir_path = os.path.dirname(keymap_path)
        self.__default_keymap_name = os.path.basename(keymap_path)

        self.__ensure_symmap(self.__default_keymap_name)

    # =====

    @exposed_http("GET", "/hid")
    async def __state_handler(self, _: Request) -> Response:
        return make_json_response(await self.__hid.get_state())

    @exposed_http("POST", "/hid/reset")
    async def __reset_handler(self, _: Request) -> Response:
        await self.__hid.reset()
        return make_json_response()

    # =====

    @exposed_http("GET", "/hid/keymaps")
    async def __keymaps_handler(self, _: Request) -> Response:
        keymaps: Set[str] = set()
        for keymap_name in os.listdir(self.__keymaps_dir_path):
            path = os.path.join(self.__keymaps_dir_path, keymap_name)
            if os.access(path, os.R_OK) and stat.S_ISREG(os.stat(path).st_mode):
                keymaps.add(keymap_name)
        return make_json_response({
            "keymaps": {
                "default": self.__default_keymap_name,
                "available": sorted(keymaps),
            },
        })

    @exposed_http("POST", "/hid/print")
    async def __print_handler(self, request: Request) -> Response:
        text = await request.text()
        limit = int(valid_int_f0(request.query.get("limit", "1024")))
        if limit > 0:
            text = text[:limit]
        symmap = self.__ensure_symmap(request.query.get("keymap", self.__default_keymap_name))
        self.__hid.send_key_events(text_to_web_keys(text, symmap))
        return make_json_response()

    def __ensure_symmap(self, keymap_name: str) -> Dict[int, Dict[int, str]]:
        keymap_name = valid_printable_filename(keymap_name, "keymap")
        path = os.path.join(self.__keymaps_dir_path, keymap_name)
        try:
            st = os.stat(path)
            if not (os.access(path, os.R_OK) and stat.S_ISREG(st.st_mode)):
                raise_error(keymap_name, "keymap")
        except Exception:
            raise_error(keymap_name, "keymap")
        return self.__inner_ensure_symmap(path, st.st_mtime)

    @functools.lru_cache(maxsize=10)
    def __inner_ensure_symmap(self, path: str, mtime: int) -> Dict[int, Dict[int, str]]:
        _ = mtime  # For LRU
        return build_symmap(path)

    # =====

    @exposed_ws("key")
    async def __ws_key_handler(self, _: WebSocketResponse, event: Dict) -> None:
        try:
            key = valid_hid_key(event["key"])
            state = valid_bool(event["state"])
        except Exception:
            return
        self.__hid.send_key_events([(key, state)])

    @exposed_ws("mouse_button")
    async def __ws_mouse_button_handler(self, _: WebSocketResponse, event: Dict) -> None:
        try:
            button = valid_hid_mouse_button(event["button"])
            state = valid_bool(event["state"])
        except Exception:
            return
        self.__hid.send_mouse_button_event(button, state)

    @exposed_ws("mouse_move")
    async def __ws_mouse_move_handler(self, _: WebSocketResponse, event: Dict) -> None:
        try:
            to_x = valid_hid_mouse_move(event["to"]["x"])
            to_y = valid_hid_mouse_move(event["to"]["y"])
        except Exception:
            return
        self.__hid.send_mouse_move_event(to_x, to_y)

    @exposed_ws("mouse_relative")
    async def __ws_mouse_relative_handler(self, _: WebSocketResponse, event: Dict) -> None:
        try:
            delta_x = valid_hid_mouse_delta(event["delta"]["x"])
            delta_y = valid_hid_mouse_delta(event["delta"]["y"])
        except Exception:
            return
        self.__hid.send_mouse_relative_event(delta_x, delta_y)

    @exposed_ws("mouse_wheel")
    async def __ws_mouse_wheel_handler(self, _: WebSocketResponse, event: Dict) -> None:
        try:
            delta_x = valid_hid_mouse_delta(event["delta"]["x"])
            delta_y = valid_hid_mouse_delta(event["delta"]["y"])
        except Exception:
            return
        self.__hid.send_mouse_wheel_event(delta_x, delta_y)

    # =====

    @exposed_http("POST", "/hid/events/send_key")
    async def __events_send_key_handler(self, request: Request) -> Response:
        key = valid_hid_key(request.query.get("key"))
        if "state" in request.query:
            state = valid_bool(request.query["state"])
            self.__hid.send_key_events([(key, state)])
        else:
            self.__hid.send_key_events([(key, True), (key, False)])
        return make_json_response()

    @exposed_http("POST", "/hid/events/send_mouse_button")
    async def __events_send_mouse_button_handler(self, request: Request) -> Response:
        button = valid_hid_mouse_button(request.query.get("button"))
        if "state" in request.query:
            state = valid_bool(request.query["state"])
            self.__hid.send_mouse_button_event(button, state)
        else:
            self.__hid.send_mouse_button_event(button, True)
            self.__hid.send_mouse_button_event(button, False)
        return make_json_response()

    @exposed_http("POST", "/hid/events/send_mouse_move")
    async def __events_send_mouse_move_handler(self, request: Request) -> Response:
        to_x = valid_hid_mouse_move(request.query.get("to_x"))
        to_y = valid_hid_mouse_move(request.query.get("to_y"))
        self.__hid.send_mouse_move_event(to_x, to_y)
        return make_json_response()

    @exposed_http("POST", "/hid/events/send_mouse_relative")
    async def __events_send_mouse_relative_handler(self, request: Request) -> Response:
        delta_x = valid_hid_mouse_delta(request.query.get("delta_x"))
        delta_y = valid_hid_mouse_delta(request.query.get("delta_y"))
        self.__hid.send_mouse_relative_event(delta_x, delta_y)
        return make_json_response()

    @exposed_http("POST", "/hid/events/send_mouse_wheel")
    async def __events_send_mouse_wheel_handler(self, request: Request) -> Response:
        delta_x = valid_hid_mouse_delta(request.query.get("delta_x"))
        delta_y = valid_hid_mouse_delta(request.query.get("delta_y"))
        self.__hid.send_mouse_wheel_event(delta_x, delta_y)
        return make_json_response()
