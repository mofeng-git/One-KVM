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


import dataclasses

from typing import Dict


# =====
@dataclasses.dataclass(frozen=True)
class SerialKey:
    code: int


@dataclasses.dataclass(frozen=True)
class Key:
    serial: SerialKey

<%! import operator %>
# =====
KEYMAP: Dict[str, Key] = {
% for km in sorted(keymap, key=operator.attrgetter("web_key")):
    "${km.web_key}": Key(serial=SerialKey(code=${km.serial_hid_code})),
% endfor
}
