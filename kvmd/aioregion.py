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


import types

from typing import Type


# =====
class RegionIsBusyError(Exception):
    def __init__(self) -> None:
        super().__init__("Performing another operation, please try again later")


class AioExclusiveRegion:
    def __init__(self, exc_type: Type[RegionIsBusyError]) -> None:
        self.__exc_type = exc_type
        self.__busy = False

    def is_busy(self) -> bool:
        return self.__busy

    def enter(self) -> None:
        if not self.__busy:
            self.__busy = True
            return
        raise self.__exc_type()

    def exit(self) -> None:
        self.__busy = False

    def __enter__(self) -> None:
        self.enter()

    def __exit__(
        self,
        _exc_type: Type[BaseException],
        _exc: BaseException,
        _tb: types.TracebackType,
    ) -> None:
        self.exit()
