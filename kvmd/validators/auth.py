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

from .basic import valid_string_list

from . import check_re_match


# =====
def valid_user(arg: Any) -> str:
    return check_re_match(arg, "username characters", r"^[a-z_][a-z0-9_-]*$")


def valid_users_list(arg: Any) -> list[str]:
    return valid_string_list(arg, subval=valid_user, name="users list")


def valid_passwd(arg: Any) -> str:
    return check_re_match(arg, "passwd characters", r"^[\x20-\x7e]*\Z$", strip=False, hide=True)


def valid_auth_token(arg: Any) -> str:
    return check_re_match(arg, "auth token", r"^[0-9a-f]{64}$", hide=True)
