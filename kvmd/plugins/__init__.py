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


import importlib
import functools
import os

from typing import Dict
from typing import Type
from typing import Any

from ..yamlconf import Option


# =====
class UnknownPluginError(Exception):
    pass


# =====
class BasePlugin:
    PLUGIN_NAME: str = ""

    def __init__(self, **_: Any) -> None:
        pass

    @classmethod
    def get_options(cls) -> Dict[str, Option]:
        return {}


# =====
def get_plugin_class(sub: str, name: str) -> Type[BasePlugin]:
    classes = _get_plugin_classes(sub)
    try:
        return classes[name]
    except KeyError:
        raise UnknownPluginError("Unknown plugin '%s/%s'" % (sub, name))


# =====
@functools.lru_cache()
def _get_plugin_classes(sub: str) -> Dict[str, Type[BasePlugin]]:
    classes: Dict[str, Type[BasePlugin]] = {}  # noqa: E701
    sub_path = os.path.join(os.path.dirname(__file__), sub)
    for file_name in os.listdir(sub_path):
        if not file_name.startswith("__") and file_name.endswith(".py"):
            module_name = file_name[:-3]
            module = importlib.import_module("kvmd.plugins.{}.{}".format(sub, module_name))
            plugin_class = getattr(module, "Plugin")
            classes[plugin_class.PLUGIN_NAME] = plugin_class
    return classes
