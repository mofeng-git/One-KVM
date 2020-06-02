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
import platform
import contextlib

from typing import Dict
from typing import Optional

import dbus  # pylint: disable=import-error
import dbus.exceptions

from ...logging import get_logger

from ...yamlconf import Section
from ...yamlconf.loader import load_yaml_file

from ... import aiotools
from ... import aioproc

from ... import __version__


# =====
class InfoManager:
    def __init__(self, global_config: Section) -> None:
        self.__global_config = global_config

    async def get_state(self) -> Dict:
        (streamer_info, meta_info, extras_info) = await asyncio.gather(
            self.__get_streamer_info(),
            self.__get_meta_info(),
            self.__get_extras_info(),
        )
        uname_info = platform.uname()  # Uname using the internal cache
        return {
            "system": {
                "kvmd": {"version": __version__},
                "streamer": streamer_info,
                "kernel": {
                    field: getattr(uname_info, field)
                    for field in ["system", "release", "version", "machine"]
                },
            },
            "meta": meta_info,
            "extras": extras_info,
        }

    # =====

    async def __get_streamer_info(self) -> Dict:
        version = ""
        features: Dict[str, bool] = {}
        try:
            path = self.__global_config.kvmd.streamer.cmd[0]
            ((_, version), (_, features_text)) = await asyncio.gather(
                aioproc.read_process([path, "--version"], err_to_null=True),
                aioproc.read_process([path, "--features"], err_to_null=True),
            )
        except Exception:
            get_logger(0).exception("Can't get streamer info")
        else:
            try:
                for line in features_text.split("\n"):
                    (status, name) = map(str.strip, line.split(" "))
                    features[name] = (status == "+")
            except Exception:
                get_logger(0).exception("Can't parse streamer features")
        return {
            "app": os.path.basename(path),
            "version": version,
            "features": features,
        }

    async def __get_meta_info(self) -> Optional[Dict]:
        try:
            return ((await aiotools.run_async(load_yaml_file, self.__global_config.kvmd.info.meta)) or {})
        except Exception:
            get_logger(0).exception("Can't parse meta")
        return None

    async def __get_extras_info(self) -> Optional[Dict]:
        return (await aiotools.run_async(self.__inner_get_extras_info))

    # =====

    def __inner_get_extras_info(self) -> Optional[Dict]:
        try:
            extras_path = self.__global_config.kvmd.info.extras
            extras: Dict[str, Dict] = {}
            for app in os.listdir(extras_path):
                if app[0] != "." and os.path.isdir(os.path.join(extras_path, app)):
                    extras[app] = load_yaml_file(os.path.join(extras_path, app, "manifest.yaml"))
                    self.__rewrite_app_daemon(extras[app])
                    self.__rewrite_app_port(extras[app])
            return extras
        except Exception:
            get_logger(0).exception("Can't parse extras")
        return None

    def __rewrite_app_daemon(self, extras: Dict) -> None:
        daemon = extras.get("daemon", "")
        if isinstance(daemon, str) and daemon.strip():
            extras["enabled"] = self.__is_daemon_enabled(daemon)

    def __rewrite_app_port(self, extras: Dict) -> None:
        port_path = extras.get("port", "")
        if isinstance(port_path, str) and port_path.strip():
            extras["port"] = 0
            config = self.__global_config
            for item in filter(None, map(str.strip, port_path.split("/"))):
                config = getattr(config, item, None)
            if isinstance(config, int):
                extras["port"] = config

    def __is_daemon_enabled(self, name: str) -> bool:
        if not name.startswith(".service"):
            name += ".service"

        try:
            with contextlib.closing(dbus.SystemBus()) as bus:
                systemd = bus.get_object("org.freedesktop.systemd1", "/org/freedesktop/systemd1")  # pylint: disable=no-member
                manager = dbus.Interface(systemd, dbus_interface="org.freedesktop.systemd1.Manager")

                try:
                    unit_proxy = bus.get_object("org.freedesktop.systemd1", manager.GetUnit(name))  # pylint: disable=no-member
                    unit_properties = dbus.Interface(unit_proxy, dbus_interface="org.freedesktop.DBus.Properties")
                    enabled = (unit_properties.Get("org.freedesktop.systemd1.Unit", "ActiveState") == "active")
                except dbus.exceptions.DBusException as err:
                    if "NoSuchUnit" not in str(err):
                        raise
                    enabled = False

                return (enabled or (manager.GetUnitFileState(name) in ["enabled", "enabled-runtime", "static", "indirect", "generated"]))
        except Exception as err:
            get_logger(0).error("Can't get info about the service %r: %s: %s", name, type(err).__name__, err)
            return True
