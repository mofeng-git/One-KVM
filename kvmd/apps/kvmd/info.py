import os
import asyncio

from typing import Dict

import dbus  # pylint: disable=import-error
import dbus.exceptions  # pylint: disable=import-error

from ...yaml import load_yaml_file


# =====
class InfoManager:
    def __init__(
        self,
        meta_path: str,
        extras_path: str,
        loop: asyncio.AbstractEventLoop,
    ) -> None:

        self.__meta_path = meta_path
        self.__extras_path = extras_path

        self.__loop = loop

    async def get_meta(self) -> Dict:
        return (await self.__loop.run_in_executor(None, load_yaml_file, self.__meta_path))

    async def get_extras(self) -> Dict:
        return (await self.__loop.run_in_executor(None, self.__sync_get_extras))

    def __sync_get_extras(self) -> Dict:
        try:
            bus = dbus.SystemBus()

            def is_enabled(daemon: str) -> bool:
                obj = bus.get_object("org.freedesktop.systemd1", "/org/freedesktop/systemd1")
                get_unit_state = obj.get_dbus_method("GetUnitFileState", "org.freedesktop.systemd1.Manager")
                return (get_unit_state(daemon + ".service") in ["enabled", "enabled-runtime", "static", "indirect", "generated"])

        except dbus.exceptions.DBusException:
            is_enabled = (lambda daemon: True)

        extras: Dict[str, Dict] = {}
        for app in os.listdir(self.__extras_path):
            if app[0] != "." and os.path.isdir(os.path.join(self.__extras_path, app)):
                extras[app] = load_yaml_file(os.path.join(self.__extras_path, app, "manifest.yaml"))
                daemon = extras[app].get("daemon", "")
                if isinstance(daemon, str) and daemon.strip():
                    extras[app]["enabled"] = is_enabled(daemon.strip())
        return extras
