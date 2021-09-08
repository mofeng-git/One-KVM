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


from typing import Tuple
from typing import Dict
from typing import Iterable
from typing import AsyncGenerator
from typing import Type
from typing import Optional

from .. import BasePlugin
from .. import get_plugin_class


# =====
class BaseHid(BasePlugin):
    def sysprep(self) -> None:
        raise NotImplementedError

    async def get_state(self) -> Dict:
        raise NotImplementedError

    async def poll_state(self) -> AsyncGenerator[Dict, None]:
        yield {}
        raise NotImplementedError

    async def reset(self) -> None:
        raise NotImplementedError

    async def cleanup(self) -> None:
        pass

    # =====

    def send_key_events(self, keys: Iterable[Tuple[str, bool]]) -> None:
        raise NotImplementedError

    def send_mouse_button_event(self, button: str, state: bool) -> None:
        raise NotImplementedError

    def send_mouse_move_event(self, to_x: int, to_y: int) -> None:
        _ = to_x
        _ = to_y

    def send_mouse_relative_event(self, delta_x: int, delta_y: int) -> None:
        _ = delta_x
        _ = delta_y

    def send_mouse_wheel_event(self, delta_x: int, delta_y: int) -> None:
        raise NotImplementedError

    def set_params(self, keyboard_output: Optional[str]=None, mouse_output: Optional[str]=None) -> None:
        _ = keyboard_output
        _ = mouse_output

    def set_connected(self, connected: bool) -> None:
        _ = connected

    def clear_events(self) -> None:
        raise NotImplementedError


# =====
def get_hid_class(name: str) -> Type[BaseHid]:
    return get_plugin_class("hid", name)  # type: ignore
