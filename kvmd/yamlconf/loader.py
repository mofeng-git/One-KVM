import os

from typing import IO
from typing import Any

import yaml
import yaml.loader
import yaml.nodes


# =====
def load_yaml_file(path: str) -> Any:
    with open(path) as yaml_file:
        try:
            return yaml.load(yaml_file, _YamlLoader)
        except Exception:
            # Reraise internal exception as standard ValueError and show the incorrect file
            raise ValueError("Incorrect YAML syntax in file '{}'".format(path))


class _YamlLoader(yaml.loader.Loader):  # pylint: disable=too-many-ancestors
    def __init__(self, yaml_file: IO) -> None:
        yaml.loader.Loader.__init__(self, yaml_file)
        self.__root = os.path.dirname(yaml_file.name)

    def include(self, node: yaml.nodes.Node) -> str:
        path = os.path.join(self.__root, self.construct_scalar(node))  # pylint: disable=no-member
        return load_yaml_file(path)


_YamlLoader.add_constructor("!include", _YamlLoader.include)  # pylint: disable=no-member
