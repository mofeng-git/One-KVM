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


from ...clients.kvmd import KvmdClient

from ... import htclient

from .. import init

from .auth import IpmiAuthManager
from .server import IpmiServer


# =====
def main(argv: (list[str] | None)=None) -> None:
    config = init(
        prog="kvmd-ipmi",
        description="IPMI to KVMD proxy",
        check_run=True,
        argv=argv,
    )[2].ipmi

    IpmiServer(
        auth_manager=IpmiAuthManager(**config.auth._unpack()),
        kvmd=KvmdClient(
            user_agent=htclient.make_user_agent("KVMD-IPMI"),
            **config.kvmd._unpack(),
        ),
        **{  # Makes mypy happy (too many arguments for IpmiServer)
            **config.server._unpack(),
            **config.sol._unpack(),
        },
    ).run()
