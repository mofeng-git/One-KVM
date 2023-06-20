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

from .. import tools

from typing import IO
from typing import Any

import yaml
import yaml.nodes
import yaml.resolver
import yaml.constructor

from .merger import yaml_merge


# =====
def load_yaml_file(path: str) -> Any:
    with open(path) as file:
        try:
            return yaml.load(file, _YamlLoader)
        except Exception as err:
            # Reraise internal exception as standard ValueError and show the incorrect file
            raise ValueError(f"Invalid YAML in the file {path!r}:\n{tools.efmt(err)}") from None


# =====
class _YamlLoader(yaml.SafeLoader):
    def __init__(self, file: IO) -> None:
        super().__init__(file)
        self.__root = os.path.dirname(file.name)

    def include(self, node: yaml.nodes.Node) -> Any:
        incs: list[str]
        if isinstance(node, yaml.nodes.SequenceNode):
            incs = [
                str(child)
                for child in self.construct_sequence(node)
                if isinstance(child, (int, float, str))
            ]
        else:  # Trying scalar for the fallback
            incs = [str(self.construct_scalar(node))]  # type: ignore
        return self.__inner_include(list(filter(None, incs)))

    def __inner_include(self, incs: list[str]) -> Any:
        tree: dict = {}
        for inc in filter(None, incs):
            assert inc, inc
            inc_path = os.path.join(self.__root, inc)
            if os.path.isdir(inc_path):
                for child in sorted(os.listdir(inc_path)):
                    child_path = os.path.join(inc_path, child)
                    if os.path.isfile(child_path) or os.path.islink(child_path):
                        yaml_merge(tree, (load_yaml_file(child_path) or {}), child_path)
            else:  # Try file
                yaml_merge(tree, (load_yaml_file(inc_path) or {}), inc_path)
        return tree


_YamlLoader.add_constructor("!include", _YamlLoader.include)


# =====
def _disable_some_bools() -> None:
    # https://stackoverflow.com/questions/36463531
    resolvers = yaml.resolver.Resolver.yaml_implicit_resolvers
    for key in "oOyYnN":
        resolvers[key] = [
            resolver
            for resolver in resolvers[key]
            if resolver[0] != "tag:yaml.org,2002:bool"
        ]
        if len(resolvers) == 0:
            del resolvers[key]


_disable_some_bools()
