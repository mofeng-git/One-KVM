import textwrap
import operator

from typing import List
from typing import Any

import yaml

from . import Section


# =====
_INDENT = 4


def make_config_dump(config: Section) -> str:
    return "\n".join(_inner_make_dump(config))


def _inner_make_dump(config: Section, _level: int=0) -> List[str]:
    lines = []
    for (key, value) in sorted(config.items(), key=operator.itemgetter(0)):
        indent = " " * _INDENT * _level
        if isinstance(value, Section):
            lines.append("{}{}:".format(indent, key))
            lines += _inner_make_dump(value, _level + 1)
            lines.append("")
        else:
            default = config._get_default(key)  # pylint: disable=protected-access
            comment = config._get_help(key)  # pylint: disable=protected-access
            if default == value:
                lines.append("{}{}: {} # {}".format(indent, key, _make_yaml(value, _level), comment))
            else:
                lines.append("{}# {}: {} # {}".format(indent, key, _make_yaml(default, _level), comment))
                lines.append("{}{}: {}".format(indent, key, _make_yaml(value, _level)))
    return lines


def _make_yaml(value: Any, level: int) -> str:
    dump = yaml.dump(value, indent=_INDENT, allow_unicode=True).replace("\n...\n", "").strip()
    if isinstance(value, dict) and dump[0] != "{" or isinstance(value, list) and dump[0] != "[":
        dump = "\n" + textwrap.indent(dump, prefix=" " * _INDENT * (level + 1))
    return dump
