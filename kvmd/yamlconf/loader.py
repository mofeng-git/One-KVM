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

from typing import IO
from typing import Any

import yaml
import yaml.nodes


# =====
def load_yaml_file(path: str) -> Any:
    with open(path) as yaml_file:
        try:
            return yaml.load(yaml_file, _YamlLoader)
        except Exception:
            # Reraise internal exception as standard ValueError and show the incorrect file
            raise ValueError("Incorrect YAML syntax in file %r" % (path))


class _YamlLoader(yaml.SafeLoader):
    def __init__(self, yaml_file: IO) -> None:
        super().__init__(yaml_file)
        self.__root = os.path.dirname(yaml_file.name)

    def include(self, node: yaml.nodes.Node) -> str:
        path = os.path.join(self.__root, self.construct_scalar(node))
        return load_yaml_file(path)


_YamlLoader.add_constructor("!include", _YamlLoader.include)
