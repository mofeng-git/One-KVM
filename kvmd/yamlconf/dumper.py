# pylint: skip-file
# infinite recursion


import operator

from typing import Tuple
from typing import List
from typing import Any

import yaml

from . import Section


# =====
def make_config_dump(config: Section) -> str:
    return "\n".join(_inner_make_dump(config))


def _inner_make_dump(config: Section, _path: Tuple[str, ...]=()) -> List[str]:
    lines = []
    for (key, value) in sorted(config.items(), key=operator.itemgetter(0)):
        indent = len(_path) * "    "
        if isinstance(value, Section):
            lines.append("{}{}:".format(indent, key))
            lines += _inner_make_dump(value, _path + (key,))
            lines.append("")
        else:
            default = config._get_default(key)  # pylint: disable=protected-access
            comment = config._get_help(key)  # pylint: disable=protected-access
            if default == value:
                lines.append("{}{}: {} # {}".format(indent, key, _make_yaml(value), comment))
            else:
                lines.append("{}# {}: {} # {}".format(indent, key, _make_yaml(default), comment))
                lines.append("{}{}: {}".format(indent, key, _make_yaml(value)))
    return lines


def _make_yaml(value: Any) -> str:
    return yaml.dump(value, allow_unicode=True).replace("\n...\n", "").strip()
