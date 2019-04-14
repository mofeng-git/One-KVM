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


import sys

from typing import Dict
from typing import Optional

import fake_rpi.RPi


# =====
class _GPIO(fake_rpi.RPi._GPIO):  # pylint: disable=protected-access
    def __init__(self) -> None:
        super().__init__()
        self.__states: Dict[int, int] = {}

    @fake_rpi.RPi.printf
    def setup(self, channel: int, state: int, initial: int=0, pull_up_down: Optional[int]=None) -> None:
        _ = state  # Makes linter happy
        _ = pull_up_down  # Makes linter happy
        self.__states[int(channel)] = int(initial)

    @fake_rpi.RPi.printf
    def output(self, channel: int, state: int) -> None:
        self.__states[int(channel)] = int(state)

    @fake_rpi.RPi.printf
    def input(self, channel: int) -> int:  # pylint: disable=arguments-differ
        return self.__states[int(channel)]

    @fake_rpi.RPi.printf
    def cleanup(self, channel: Optional[int]=None) -> None:  # pylint: disable=arguments-differ
        _ = channel  # Makes linter happy
        self.__states = {}


# =====
fake_rpi.RPi.GPIO = _GPIO()
sys.modules["RPi"] = fake_rpi.RPi
