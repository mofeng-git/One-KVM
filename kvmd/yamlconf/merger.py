# ========================================================================== #
#                                                                            #
#    KVMD - The main PiKVM daemon.                                           #
#                                                                            #
#    Copyright (C) 2018-2022  Maxim Devaev <mdevaev@gmail.com>               #
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

from typing import Optional


# =====
def _merge(dest: dict, src: dict) -> None:
    for key in src:
        if key in dest:
            if isinstance(dest[key], dict) and isinstance(src[key], dict):
                _merge(dest[key], src[key])
                continue
        dest[key] = src[key]


def yaml_merge(dest: dict, src: dict, source_name: Optional[str]=None) -> None:
    """ Merges the source dictionary into the destination dictionary. """

    # Checking if destination is None
    if dest is None:
        # We can't merge into a None
        raise ValueError(f"Could not merge {source_name} into None. The destination cannot be None")

    # Checking if source is None or empty
    if src is None:
        # If src is None or empty, there's nothing to merge
        return

    _merge(dest, src)
