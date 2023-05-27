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


from typing import Any

from . import raise_error
from . import check_string_in_list
from . import check_re_match
from . import check_len


# =====
def valid_ugpio_driver(arg: Any, variants: (set[str] | None)=None) -> str:
    name = "GPIO driver"
    arg = check_len(check_re_match(arg, name, r"^[a-zA-Z_][a-zA-Z0-9_-]*$"), name, 255)
    if variants is not None:
        arg = check_string_in_list(arg, f"configured {name}", variants, False)
    return arg


def valid_ugpio_channel(arg: Any) -> str:
    name = "GPIO channel"
    return check_len(check_re_match(arg, name, r"^[a-zA-Z_][a-zA-Z0-9_.-]*$"), name, 255)


def valid_ugpio_mode(arg: Any, variants: set[str]) -> str:
    return check_string_in_list(arg, "GPIO driver's pin mode", variants)


def valid_ugpio_view_title(arg: Any) -> (str | list[str]):
    return (list(map(str, arg)) if isinstance(arg, list) else str(arg))


def valid_ugpio_view_table(arg: Any) -> list[list[str]]:  # pylint: disable=inconsistent-return-statements
    try:
        return [list(map(str, row)) for row in list(arg)]
    except Exception:
        raise_error("<skipped>", "GPIO view table")
