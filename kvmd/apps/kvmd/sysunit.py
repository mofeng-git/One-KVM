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


import contextlib

from typing import Tuple
from typing import Optional

import dbus  # pylint: disable=import-error
import dbus.exceptions

from ...logging import get_logger

from ... import tools


# =====
def get_service_status(name: str) -> Optional[Tuple[bool, bool]]:
    if not name.endswith(".service"):
        name += ".service"
    try:
        with contextlib.closing(dbus.SystemBus()) as bus:
            systemd = bus.get_object("org.freedesktop.systemd1", "/org/freedesktop/systemd1")  # pylint: disable=no-member
            manager = dbus.Interface(systemd, dbus_interface="org.freedesktop.systemd1.Manager")
            try:
                unit_proxy = bus.get_object("org.freedesktop.systemd1", manager.GetUnit(name))  # pylint: disable=no-member
                unit_properties = dbus.Interface(unit_proxy, dbus_interface="org.freedesktop.DBus.Properties")
                started = (unit_properties.Get("org.freedesktop.systemd1.Unit", "ActiveState") == "active")
            except dbus.exceptions.DBusException as err:
                if "NoSuchUnit" not in str(err):
                    raise
                started = False
            enabled = (manager.GetUnitFileState(name) in ["enabled", "enabled-runtime", "static", "indirect", "generated"])
            return (enabled, started)
    except Exception as err:
        get_logger(0).error("Can't get info about the service %r: %s", name, tools.efmt(err))
        return None
