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
import asyncio
import contextlib

from typing import Dict

import dbus  # pylint: disable=import-error

from ...logging import get_logger

from ...yamlconf.loader import load_yaml_file


# =====
class InfoManager:
    def __init__(
        self,
        meta_path: str,
        extras_path: str,
    ) -> None:

        self.__meta_path = meta_path
        self.__extras_path = extras_path

    async def get_meta(self) -> Dict:
        return (await asyncio.get_running_loop().run_in_executor(None, load_yaml_file, self.__meta_path))

    async def get_extras(self) -> Dict:
        return (await asyncio.get_running_loop().run_in_executor(None, self.__inner_get_extras))

    def __inner_get_extras(self) -> Dict:
        extras: Dict[str, Dict] = {}
        for app in os.listdir(self.__extras_path):
            if app[0] != "." and os.path.isdir(os.path.join(self.__extras_path, app)):
                extras[app] = load_yaml_file(os.path.join(self.__extras_path, app, "manifest.yaml"))
                daemon = extras[app].get("daemon", "")
                if isinstance(daemon, str) and daemon.strip():
                    extras[app]["enabled"] = self.__is_daemon_enabled(daemon)
        return extras

    def __is_daemon_enabled(self, name: str) -> bool:
        if not name.startswith(".service"):
            name += ".service"
        try:
            with contextlib.closing(dbus.SystemBus()) as bus:
                systemd = bus.get_object("org.freedesktop.systemd1", "/org/freedesktop/systemd1")  # pylint: disable=no-member
                get_unit_state = systemd.get_dbus_method("GetUnitFileState", "org.freedesktop.systemd1.Manager")
                return (get_unit_state(name) in ["enabled", "enabled-runtime", "static", "indirect", "generated"])
        except Exception as err:
            get_logger(0).error("Can't get info about the service %r: %s: %s", name, type(err).__name__, str(err))
            return True
