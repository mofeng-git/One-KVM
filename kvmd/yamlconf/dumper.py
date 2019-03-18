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
