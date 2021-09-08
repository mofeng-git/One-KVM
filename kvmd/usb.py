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


import os

from typing import Tuple

from .logging import get_logger

from . import env


# =====
def find_udc(udc: str) -> Tuple[str, str]:
    path = f"{env.SYSFS_PREFIX}/sys/class/udc"
    candidates = sorted(os.listdir(path))
    if not udc:
        if len(candidates) == 0:
            raise RuntimeError("Can't find any UDC")
        udc = candidates[0]
    elif udc not in candidates:
        raise RuntimeError(f"Can't find selected UDC: {udc}")
    driver = os.path.basename(os.readlink(os.path.join(path, udc, "device/driver")))
    return (udc, driver)  # (fe980000.usb, dwc2)


class UsbDeviceController:
    def __init__(self, udc: str) -> None:
        self.__udc = udc
        self.__state_path = ""

    def find(self) -> None:
        udc = find_udc(self.__udc)[0]
        self.__state_path = os.path.join(f"{env.SYSFS_PREFIX}/sys/class/udc", udc, "state")
        get_logger().info("Using UDC %s", udc)

    def can_operate(self) -> bool:
        assert self.__state_path
        with open(self.__state_path, "r") as state_file:
            # https://www.maxlinear.com/Files/Documents/an213_033111.pdf
            return (state_file.read().strip().lower() == "configured")
