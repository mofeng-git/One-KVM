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


import os
import stat
import functools
import struct

from typing import Callable

from aiohttp.web import Request
from aiohttp.web import Response

from ....mouse import MouseRange

from ....keyboard.keysym import build_symmap
from ....keyboard.printer import text_to_web_keys

from ....htserver import exposed_http
from ....htserver import exposed_ws
from ....htserver import make_json_response
from ....htserver import WsSession

from ....plugins.hid import BaseHid

from ....validators import raise_error
from ....validators.basic import valid_bool
from ....validators.basic import valid_int_f0
from ....validators.os import valid_printable_filename
from ....validators.hid import valid_hid_keyboard_output
from ....validators.hid import valid_hid_mouse_output
from ....validators.hid import valid_hid_key
from ....validators.hid import valid_hid_mouse_move
from ....validators.hid import valid_hid_mouse_button
from ....validators.hid import valid_hid_mouse_delta


# =====
class HidApi:
    def __init__(
        self,
        hid: BaseHid,

        keymap_path: str,
        ignore_keys: list[str],

        mouse_x_range: tuple[int, int],
        mouse_y_range: tuple[int, int],
    ) -> None:

        self.__hid = hid

        self.__keymaps_dir_path = os.path.dirname(keymap_path)
        self.__default_keymap_name = os.path.basename(keymap_path)
        self.__ensure_symmap(self.__default_keymap_name)

        self.__ignore_keys = ignore_keys

        self.__mouse_x_range = mouse_x_range
        self.__mouse_y_range = mouse_y_range

    # =====

    @exposed_http("GET", "/hid")
    async def __state_handler(self, _: Request) -> Response:
        return make_json_response(await self.__hid.get_state())

    @exposed_http("POST", "/hid/set_params")
    async def __set_params_handler(self, request: Request) -> Response:
        params = {
            key: validator(request.query.get(key))
            for (key, validator) in [
                ("keyboard_output", valid_hid_keyboard_output),
                ("mouse_output", valid_hid_mouse_output),
                ("jiggler", valid_bool),
            ]
            if request.query.get(key) is not None
        }
        self.__hid.set_params(**params)  # type: ignore
        return make_json_response()

    @exposed_http("POST", "/hid/set_connected")
    async def __set_connected_handler(self, request: Request) -> Response:
        self.__hid.set_connected(valid_bool(request.query.get("connected")))
        return make_json_response()

    @exposed_http("POST", "/hid/reset")
    async def __reset_handler(self, _: Request) -> Response:
        await self.__hid.reset()
        return make_json_response()

    # =====

    async def get_keymaps(self) -> dict:  # Ugly hack to generate hid_keymaps_state (see server.py)
        keymaps: set[str] = set()
        for keymap_name in os.listdir(self.__keymaps_dir_path):
            path = os.path.join(self.__keymaps_dir_path, keymap_name)
            if os.access(path, os.R_OK) and stat.S_ISREG(os.stat(path).st_mode):
                keymaps.add(keymap_name)
        return {
            "keymaps": {
                "default": self.__default_keymap_name,
                "available": sorted(keymaps),
            },
        }

    @exposed_http("GET", "/hid/keymaps")
    async def __keymaps_handler(self, _: Request) -> Response:
        return make_json_response(await self.get_keymaps())

    @exposed_http("POST", "/hid/print")
    async def __print_handler(self, request: Request) -> Response:
        text = await request.text()
        limit = int(valid_int_f0(request.query.get("limit", 1024)))
        if limit > 0:
            text = text[:limit]
        symmap = self.__ensure_symmap(request.query.get("keymap", self.__default_keymap_name))
        self.__hid.send_key_events(text_to_web_keys(text, symmap))
        return make_json_response()

    def __ensure_symmap(self, keymap_name: str) -> dict[int, dict[int, str]]:
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
    def __inner_ensure_symmap(self, path: str, mod_ts: int) -> dict[int, dict[int, str]]:
        _ = mod_ts  # For LRU
        return build_symmap(path)

    # =====

    @exposed_ws(1)
    async def __ws_bin_key_handler(self, _: WsSession, data: bytes) -> None:
        try:
            key = valid_hid_key(data[1:].decode("ascii"))
            state = valid_bool(data[0])
        except Exception:
            return
        if key not in self.__ignore_keys:
            self.__hid.send_key_events([(key, state)])

    @exposed_ws(2)
    async def __ws_bin_mouse_button_handler(self, _: WsSession, data: bytes) -> None:
        try:
            button = valid_hid_mouse_button(data[1:].decode("ascii"))
            state = valid_bool(data[0])
        except Exception:
            return
        self.__hid.send_mouse_button_event(button, state)

    @exposed_ws(3)
    async def __ws_bin_mouse_move_handler(self, _: WsSession, data: bytes) -> None:
        try:
            (to_x, to_y) = struct.unpack(">hh", data)
            to_x = valid_hid_mouse_move(to_x)
            to_y = valid_hid_mouse_move(to_y)
        except Exception:
            return
        self.__send_mouse_move_event(to_x, to_y)

    @exposed_ws(4)
    async def __ws_bin_mouse_relative_handler(self, _: WsSession, data: bytes) -> None:
        self.__process_ws_bin_delta_request(data, self.__hid.send_mouse_relative_event)

    @exposed_ws(5)
    async def __ws_bin_mouse_wheel_handler(self, _: WsSession, data: bytes) -> None:
        self.__process_ws_bin_delta_request(data, self.__hid.send_mouse_wheel_event)

    def __process_ws_bin_delta_request(self, data: bytes, handler: Callable[[int, int], None]) -> None:
        try:
            squash = valid_bool(data[0])
            data = data[1:]
            deltas: list[tuple[int, int]] = []
            for index in range(0, len(data), 2):
                (delta_x, delta_y) = struct.unpack(">bb", data[index:index + 2])
                deltas.append((valid_hid_mouse_delta(delta_x), valid_hid_mouse_delta(delta_y)))
        except Exception:
            return
        self.__send_mouse_delta_event(deltas, squash, handler)

    # =====

    @exposed_ws("key")
    async def __ws_key_handler(self, _: WsSession, event: dict) -> None:
        try:
            key = valid_hid_key(event["key"])
            state = valid_bool(event["state"])
        except Exception:
            return
        if key not in self.__ignore_keys:
            self.__hid.send_key_events([(key, state)])

    @exposed_ws("mouse_button")
    async def __ws_mouse_button_handler(self, _: WsSession, event: dict) -> None:
        try:
            button = valid_hid_mouse_button(event["button"])
            state = valid_bool(event["state"])
        except Exception:
            return
        self.__hid.send_mouse_button_event(button, state)

    @exposed_ws("mouse_move")
    async def __ws_mouse_move_handler(self, _: WsSession, event: dict) -> None:
        try:
            to_x = valid_hid_mouse_move(event["to"]["x"])
            to_y = valid_hid_mouse_move(event["to"]["y"])
        except Exception:
            return
        self.__send_mouse_move_event(to_x, to_y)

    @exposed_ws("mouse_relative")
    async def __ws_mouse_relative_handler(self, _: WsSession, event: dict) -> None:
        self.__process_ws_delta_event(event, self.__hid.send_mouse_relative_event)

    @exposed_ws("mouse_wheel")
    async def __ws_mouse_wheel_handler(self, _: WsSession, event: dict) -> None:
        self.__process_ws_delta_event(event, self.__hid.send_mouse_wheel_event)

    def __process_ws_delta_event(self, event: dict, handler: Callable[[int, int], None]) -> None:
        try:
            raw_delta = event["delta"]
            deltas = [
                (valid_hid_mouse_delta(delta["x"]), valid_hid_mouse_delta(delta["y"]))
                for delta in (raw_delta if isinstance(raw_delta, list) else [raw_delta])
            ]
            squash = valid_bool(event.get("squash", False))
        except Exception:
            return
        self.__send_mouse_delta_event(deltas, squash, handler)

    # =====

    @exposed_http("POST", "/hid/events/send_key")
    async def __events_send_key_handler(self, request: Request) -> Response:
        key = valid_hid_key(request.query.get("key"))
        if key not in self.__ignore_keys:
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
        self.__send_mouse_move_event(to_x, to_y)
        return make_json_response()

    @exposed_http("POST", "/hid/events/send_mouse_relative")
    async def __events_send_mouse_relative_handler(self, request: Request) -> Response:
        return self.__process_http_delta_event(request, self.__hid.send_mouse_relative_event)

    @exposed_http("POST", "/hid/events/send_mouse_wheel")
    async def __events_send_mouse_wheel_handler(self, request: Request) -> Response:
        return self.__process_http_delta_event(request, self.__hid.send_mouse_wheel_event)

    def __process_http_delta_event(self, request: Request, handler: Callable[[int, int], None]) -> Response:
        delta_x = valid_hid_mouse_delta(request.query.get("delta_x"))
        delta_y = valid_hid_mouse_delta(request.query.get("delta_y"))
        handler(delta_x, delta_y)
        return make_json_response()

    # =====

    def __send_mouse_move_event(self, to_x: int, to_y: int) -> None:
        if self.__mouse_x_range != MouseRange.RANGE:
            to_x = MouseRange.remap(to_x, *self.__mouse_x_range)
        if self.__mouse_y_range != MouseRange.RANGE:
            to_y = MouseRange.remap(to_y, *self.__mouse_y_range)
        self.__hid.send_mouse_move_event(to_x, to_y)

    def __send_mouse_delta_event(
        self,
        deltas: list[tuple[int, int]],
        squash: bool,
        handler: Callable[[int, int], None],
    ) -> None:

        if squash:
            prev = (0, 0)
            for cur in deltas:
                if abs(prev[0] + cur[0]) > 127 or abs(prev[1] + cur[1]) > 127:
                    handler(*prev)
                    prev = cur
                else:
                    prev = (prev[0] + cur[0], prev[1] + cur[1])
            if prev[0] or prev[1]:
                handler(*prev)
        else:
            for xy in deltas:
                handler(*xy)
