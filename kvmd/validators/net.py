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
import ssl

from typing import List
from typing import Callable
from typing import Any

from . import ValidatorError
from . import raise_error
from . import check_re_match
from . import check_any

from .basic import valid_number
from .basic import valid_stripped_string_not_empty
from .basic import valid_string_list


# =====
def valid_ip_or_host(arg: Any) -> str:
    name = "IPv4/6 address or RFC-1123 hostname"
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
    versions: List[str] = []
    if v4:
        validators.append(lambda arg: str(ipaddress.IPv4Address(arg)))
        versions.append("4")
    if v6:
        validators.append(lambda arg: str(ipaddress.IPv6Address(arg)))
        versions.append("6")
    name = f"IPv{'/'.join(versions)} address"
    return check_any(
        arg=valid_stripped_string_not_empty(arg, name),
        name=name,
        validators=validators,
    )


def valid_net(arg: Any, v4: bool=True, v6: bool=True) -> str:
    assert v4 or v6
    validators: List[Callable] = []
    versions: List[str] = []
    if v4:
        validators.append(lambda arg: str(ipaddress.IPv4Network(arg)))
        versions.append("4")
    if v6:
        validators.append(lambda arg: str(ipaddress.IPv6Network(arg)))
        versions.append("6")
    name = f"IPv{'/'.join(versions)} network"
    if "/" not in str(arg):
        raise_error(arg, name)
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
    return int(valid_number(arg, min=0, max=65535, name="network port"))


def valid_ports_list(arg: Any) -> List[int]:
    return list(map(int, valid_string_list(arg, subval=valid_port, name="ports list")))


def valid_mac(arg: Any) -> str:
    pattern = ":".join([r"[0-9a-fA-F]{2}"] * 6)
    return check_re_match(arg, "MAC address", pattern).lower()


def valid_ssl_ciphers(arg: Any) -> str:
    name = "SSL ciphers"
    arg = valid_stripped_string_not_empty(arg, name)
    try:
        ssl.SSLContext().set_ciphers(arg)
    except Exception as err:
        raise ValidatorError(f"The argument {arg!r} is not a valid {name}: {err}")
    return arg
