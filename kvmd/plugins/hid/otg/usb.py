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

from ....logging import get_logger

from .... import env


# =====
class UsbDeviceController:
    # Проблема в том, что устройство может отвечать EAGAIN или ESHUTDOWN,
    # если оно было отключено физически. См:
    #    - https://github.com/raspberrypi/linux/issues/3870
    #    - https://github.com/raspberrypi/linux/pull/3151
    # Так что нам нужно проверять состояние контроллера, чтобы не спамить
    # в устройство и отслеживать его состояние.

    def __init__(self, udc: str) -> None:
        self.__udc = udc
        self.__state_path = ""

    def find(self) -> None:
        logger = get_logger()

        path = f"{env.SYSFS_PREFIX}/sys/class/udc"
        try:
            candidates = sorted(os.listdir(path))
        except Exception as err:
            logger.error("Can't list %s: %s: %s: ignored", path, type(err).__name__, err)
            return

        udc = ""
        if not self.__udc:
            if len(candidates) == 0:
                logger.error("Can't find any UDC: ignored")
            else:
                udc = candidates[0]
        elif self.__udc not in candidates:
            logger.error("Can't find selected UDC: %s: ignored", self.__udc)
        else:
            udc = self.__udc

        if udc:
            get_logger().info("Using UDC %s", udc)
            self.__state_path = os.path.join(path, udc, "state")

    def can_operate(self) -> bool:
        if self.__state_path:
            try:
                with open(self.__state_path, "r") as state_file:
                    # https://www.maxlinear.com/Files/Documents/an213_033111.pdf
                    return (state_file.read().strip().lower() == "configured")
            except Exception:
                pass
        return True  # При ошибке лучше прикинуться работающим, мало ли что
