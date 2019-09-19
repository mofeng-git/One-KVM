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


from typing import List
from typing import Optional

from .. import init

from .auth import IpmiAuthManager
from .server import IpmiServer


# =====
def main(argv: Optional[List[str]]=None) -> None:
    config = init(
        prog="kvmd-ipmi",
        description="IPMI to KVMD proxy",
        argv=argv,
    )[2].ipmi

    # pylint: disable=protected-access
    IpmiServer(
        auth_manager=IpmiAuthManager(**config.auth._unpack()),
        **{  # Dirty mypy hack
            **config.server._unpack(),
            **config.kvmd._unpack(),
        },
    ).run()  # type: ignore
