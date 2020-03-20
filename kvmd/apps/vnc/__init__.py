# ========================================================================== #
#                                                                            #
#    KVMD - The main Pi-KVM daemon.                                          #
#                                                                            #
#    Copyright (C) 2020  Maxim Devaev <mdevaev@gmail.com>                    #
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

from .kvmd import KvmdClient
from .streamer import StreamerClient
from .server import VncServer
from .keysym import build_symmap


# =====
def main(argv: Optional[List[str]]=None) -> None:
    config = init(
        prog="kvmd-vnc",
        description="VNC to KVMD proxy",
        argv=argv,
    )[2].vnc

    # pylint: disable=protected-access
    VncServer(
        kvmd=KvmdClient(**config.kvmd._unpack()),
        streamer=StreamerClient(**config.streamer._unpack()),
        symmap=build_symmap(config.keymap),
        **config.server._unpack(),
    ).run()
