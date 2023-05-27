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


import importlib
import functools

from typing import Any


# =====
class UnknownPluginError(Exception):
    pass


# =====
class BasePlugin:
    def __init__(self, **_: Any) -> None:
        pass  # pragma: nocover

    @classmethod
    def get_plugin_name(cls) -> str:
        name = cls.__module__
        return name[name.rindex(".") + 1:]

    @classmethod
    def get_plugin_options(cls) -> dict:
        return {}  # pragma: nocover


@functools.lru_cache()
def get_plugin_class(sub: str, name: str) -> type[BasePlugin]:
    assert sub
    assert name
    if name.startswith("_"):
        raise UnknownPluginError(f"Unknown plugin '{sub}/{name}'")
    try:
        module = importlib.import_module(f"kvmd.plugins.{sub}.{name}")
    except ModuleNotFoundError:
        raise UnknownPluginError(f"Unknown plugin '{sub}/{name}'")
    return getattr(module, "Plugin")
