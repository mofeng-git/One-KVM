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


import ipaddress

from typing import List
from typing import Callable
from typing import Any

from . import check_re_match
from . import check_any

from .basic import valid_number
from .basic import valid_stripped_string_not_empty


# =====
def valid_ip_or_host(arg: Any) -> str:
    name = "IP address or RFC-1123 hostname"
    return check_any(
        arg=valid_stripped_string_not_empty(arg, name),
        name=name,
        validators=[
            valid_ip,
            valid_rfc_host,
        ],
    )


def valid_ip(arg: Any, v4: bool=True, v6: bool=True) -> str:
    assert v4 or v6
    validators: List[Callable] = []
    if v4:
        validators.append(lambda arg: str(ipaddress.IPv4Address(arg)))
    if v6:
        validators.append(lambda arg: str(ipaddress.IPv6Address(arg)))
    name = "IP address"
    return check_any(
        arg=valid_stripped_string_not_empty(arg, name),
        name=name,
        validators=validators,
    )


def valid_rfc_host(arg: Any) -> str:
    # http://stackoverflow.com/questions/106179/regular-expression-to-match-hostname-or-ip-address
    pattern = r"^(([a-zA-Z0-9]|[a-zA-Z0-9][a-zA-Z0-9\-]*[a-zA-Z0-9])\.)*" \
              r"([A-Za-z0-9]|[A-Za-z0-9][A-Za-z0-9\-]*[A-Za-z0-9])$"
    return check_re_match(arg, "RFC-1123 hostname", pattern)


def valid_port(arg: Any) -> int:
    return int(valid_number(arg, min=0, max=65535, name="TCP/UDP port"))


def valid_mac(arg: Any) -> str:
    pattern = ":".join([r"[0-9a-fA-F]{2}"] * 6)
    return check_re_match(arg, "MAC address", pattern).lower()
