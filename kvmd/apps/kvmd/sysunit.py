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


import types

import dbus_next
import dbus_next.aio
import dbus_next.aio.proxy_object
import dbus_next.introspection
import dbus_next.errors


# =====
class SystemdUnitInfo:
    def __init__(self) -> None:
        self.__bus: (dbus_next.aio.MessageBus | None) = None
        self.__intr: (dbus_next.introspection.Node | None) = None
        self.__manager: (dbus_next.aio.proxy_object.ProxyInterface | None) = None

    async def get_status(self, name: str) -> tuple[bool, bool]:
        assert self.__bus is not None
        assert self.__intr is not None
        assert self.__manager is not None

        if not name.endswith(".service"):
            name += ".service"

        try:
            unit_p = await self.__manager.call_get_unit(name)  # type: ignore
            unit = self.__bus.get_proxy_object("org.freedesktop.systemd1", unit_p, self.__intr)
            unit_props = unit.get_interface("org.freedesktop.DBus.Properties")
            started = ((await unit_props.call_get("org.freedesktop.systemd1.Unit", "ActiveState")).value == "active")  # type: ignore
        except dbus_next.errors.DBusError as err:
            if err.type != "org.freedesktop.systemd1.NoSuchUnit":
                raise
            started = False
        enabled = ((await self.__manager.call_get_unit_file_state(name)) in [  # type: ignore
            "enabled",
            "enabled-runtime",
            "static",
            "indirect",
            "generated",
        ])
        return (enabled, started)

    async def open(self) -> None:
        self.__bus = await dbus_next.aio.MessageBus(bus_type=dbus_next.BusType.SYSTEM).connect()
        self.__intr = await self.__bus.introspect("org.freedesktop.systemd1", "/org/freedesktop/systemd1")
        systemd = self.__bus.get_proxy_object("org.freedesktop.systemd1", "/org/freedesktop/systemd1", self.__intr)
        self.__manager = systemd.get_interface("org.freedesktop.systemd1.Manager")

    async def __aenter__(self) -> "SystemdUnitInfo":
        await self.open()
        return self

    async def close(self) -> None:
        try:
            if self.__bus is not None:
                self.__bus.disconnect()
                await self.__bus.wait_for_disconnect()
        except Exception:
            pass
        self.__manager = None
        self.__intr = None
        self.__bus = None

    async def __aexit__(
        self,
        _exc_type: type[BaseException],
        _exc: BaseException,
        _tb: types.TracebackType,
    ) -> None:

        await self.close()
