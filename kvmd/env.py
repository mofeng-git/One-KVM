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


# =====
# XXX: Don't use these variables for any purpose other than testing.
# It can be removed at any time.

GPIO_DEVICE_PATH = str(os.getenv("KVMD_GPIO_DEVICE_PATH", "/dev/gpiochip0")).strip()

SYSFS_PREFIX = str(os.getenv("KVMD_SYSFS_PREFIX", "")).strip()
PROCFS_PREFIX = str(os.getenv("KVMD_PROCFS_PREFIX", "")).strip()
