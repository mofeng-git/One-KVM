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


from typing import List

from ....yamlconf import Section

from .base import BaseInfoSubmanager
from .system import SystemInfoSubmanager
from .meta import MetaInfoSubmanager
from .extras import ExtrasInfoSubmanager
from .hw import HwInfoSubmanager


# =====
class InfoManager:
    def __init__(self, config: Section) -> None:
        self.__subs = {
            "system": SystemInfoSubmanager(config.kvmd.streamer.cmd),
            "meta": MetaInfoSubmanager(config.kvmd.info.meta),
            "extras": ExtrasInfoSubmanager(config),
            "hw": HwInfoSubmanager(**config.kvmd.info.hw._unpack()),
        }

    def get_subs(self) -> List[str]:
        return list(self.__subs)

    def get_submanager(self, name: str) -> BaseInfoSubmanager:
        return self.__subs[name]
