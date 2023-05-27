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
import re
import asyncio

from ....logging import get_logger

from ....yamlconf import Section
from ....yamlconf.loader import load_yaml_file

from .... import tools
from .... import aiotools

from .. import sysunit

from .base import BaseInfoSubmanager


# =====
class ExtrasInfoSubmanager(BaseInfoSubmanager):
    def __init__(self, global_config: Section) -> None:
        self.__global_config = global_config

    async def get_state(self) -> (dict | None):
        try:
            sui = sysunit.SystemdUnitInfo()
            await sui.open()
        except Exception as err:
            get_logger(0).error("Can't open systemd bus to get extras state: %s", tools.efmt(err))
            sui = None
        try:
            extras: dict[str, dict] = {}
            for extra in (await asyncio.gather(*[
                self.__read_extra(sui, name)
                for name in os.listdir(self.__get_extras_path())
                if name[0] != "." and os.path.isdir(self.__get_extras_path(name))
            ])):
                extras.update(extra)
            return extras
        except Exception:
            get_logger(0).exception("Can't read extras")
            return None
        finally:
            if sui is not None:
                await aiotools.shield_fg(sui.close())

    def __get_extras_path(self, *parts: str) -> str:
        return os.path.join(self.__global_config.kvmd.info.extras, *parts)

    async def __read_extra(self, sui: (sysunit.SystemdUnitInfo | None), name: str) -> dict:
        try:
            extra = await aiotools.run_async(load_yaml_file, self.__get_extras_path(name, "manifest.yaml"))
            await self.__rewrite_app_daemon(sui, extra)
            self.__rewrite_app_port(extra)
            return {re.sub(r"[^a-zA-Z0-9_]+", "_", name): extra}
        except Exception:
            get_logger(0).exception("Can't read extra %r", name)
            return {}

    async def __rewrite_app_daemon(self, sui: (sysunit.SystemdUnitInfo | None), extra: dict) -> None:
        daemon = extra.get("daemon", "")
        if isinstance(daemon, str) and daemon.strip():
            extra["enabled"] = extra["started"] = False
            if sui is not None:
                try:
                    (extra["enabled"], extra["started"]) = await sui.get_status(daemon)
                except Exception as err:
                    get_logger(0).error("Can't get info about the service %r: %s", daemon, tools.efmt(err))

    def __rewrite_app_port(self, extra: dict) -> None:
        port_path = extra.get("port", "")
        if isinstance(port_path, str) and port_path.strip():
            extra["port"] = 0
            config = self.__global_config
            for item in filter(None, map(str.strip, port_path.split("/"))):
                config = getattr(config, item, None)  # type: ignore
            if isinstance(config, int):
                extra["port"] = config
