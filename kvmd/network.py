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


import socket
import errno


# =====
def is_ipv6_enabled() -> bool:
    if not socket.has_ipv6:
        # If the socket library has no support for IPv6,
        # then the question is moot as we can't use IPv6 anyways.
        return False
    try:
        with socket.socket(socket.AF_INET6, socket.SOCK_STREAM) as sock:
            sock.bind(("::1", 0))
        return True
    except OSError as err:
        if err.errno in [errno.EADDRNOTAVAIL, errno.EAFNOSUPPORT]:
            return False
        if err.errno == errno.EADDRINUSE:
            return True
        raise


def get_listen_host(host: str) -> str:
    if len(host) == 0:
        return ("::" if is_ipv6_enabled() else "0.0.0.0")
    return host
