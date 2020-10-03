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
import errno

from .... import env

from .. import MsdOperationError


# =====
class MsdDriveLockedError(MsdOperationError):
    def __init__(self) -> None:
        super().__init__("MSD drive is locked on IO operation")


# =====
class Drive:
    def __init__(self, gadget: str, instance: int, lun: int) -> None:
        self.__path = os.path.join(
            f"{env.SYSFS_PREFIX}/sys/kernel/config/usb_gadget",
            gadget,
            f"functions/mass_storage.usb{instance}/lun.{lun}",
        )

    def get_sysfs_path(self) -> str:
        return self.__path

    # =====

    def set_image_path(self, path: str) -> None:
        self.__set_param("file", path)

    def get_image_path(self) -> str:
        return self.__get_param("file")

    def set_cdrom_flag(self, flag: bool) -> None:
        self.__set_param("cdrom", str(int(flag)))

    def get_cdrom_flag(self) -> bool:
        return bool(int(self.__get_param("cdrom")))

    def set_rw_flag(self, flag: bool) -> None:
        self.__set_param("ro", str(int(not flag)))

    def get_rw_flag(self) -> bool:
        return (not int(self.__get_param("ro")))

    # =====

    def __get_param(self, param: str) -> str:
        with open(os.path.join(self.__path, param)) as param_file:
            return param_file.read().strip()

    def __set_param(self, param: str, value: str) -> None:
        try:
            with open(os.path.join(self.__path, param), "w") as param_file:
                param_file.write(value + "\n")
        except OSError as err:
            if err.errno == errno.EBUSY:
                raise MsdDriveLockedError()
            raise
