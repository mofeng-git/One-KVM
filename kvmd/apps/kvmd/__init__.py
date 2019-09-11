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

from ...logging import get_logger

from ... import gpio

from ...plugins.hid import get_hid_class
from ...plugins.atx import get_atx_class

from .. import init

from .auth import AuthManager
from .info import InfoManager
from .logreader import LogReader
from .msd import MassStorageDevice
from .streamer import Streamer
from .server import Server


# =====
def main(argv: Optional[List[str]]=None) -> None:
    config = init(
        prog="kvmd",
        description="The main Pi-KVM daemon",
        sections=["logging", "kvmd"],
        argv=argv,
    )[2].kvmd

    with gpio.bcm():
        # pylint: disable=protected-access
        Server(
            auth_manager=AuthManager(
                internal_type=config.auth.internal.type,
                internal_kwargs=config.auth.internal._unpack(ignore=["type", "force_users"]),
                external_type=config.auth.external.type,
                external_kwargs=(config.auth.external._unpack(ignore=["type"]) if config.auth.external.type else {}),
                force_internal_users=config.auth.internal.force_users,
            ),
            info_manager=InfoManager(**config.info._unpack()),
            log_reader=LogReader(),

            hid=get_hid_class(config.hid.type)(**config.hid._unpack(ignore=["type"])),
            atx=get_atx_class(config.atx.type)(**config.atx._unpack(ignore=["type"])),
            msd=MassStorageDevice(**config.msd._unpack()),
            streamer=Streamer(**config.streamer._unpack()),
        ).run(**config.server._unpack())

    get_logger(0).info("Bye-bye")
