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

from . import env


# =====
def find_udc(udc: str) -> str:
    path = f"{env.SYSFS_PREFIX}/sys/class/udc"
    candidates = sorted(os.listdir(path))
    if not udc:
        if len(candidates) == 0:
            raise RuntimeError("Can't find any UDC")
        udc = candidates[0]
    elif udc not in candidates:
        raise RuntimeError(f"Can't find selected UDC: {udc}")
    return udc  # fe980000.usb


# =====
U_STATE = "state"


def get_udc_path(udc: str, *parts: str) -> str:
    return os.path.join(f"{env.SYSFS_PREFIX}/sys/class/udc", udc, *parts)


# =====
G_UDC = "UDC"
G_FUNCTIONS = "functions"
G_PROFILE_NAME = "c.1"
G_PROFILE = f"configs/{G_PROFILE_NAME}"


def get_gadget_path(gadget: str, *parts: str) -> str:
    return os.path.join(f"{env.SYSFS_PREFIX}/sys/kernel/config/usb_gadget", gadget, *parts)
