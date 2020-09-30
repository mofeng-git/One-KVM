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

from ...clients.kvmd import KvmdClient
from ...clients.streamer import StreamerClient

from ... import htclient

from .. import init

from .vncauth import VncAuthManager
from .server import VncServer


# =====
def main(argv: Optional[List[str]]=None) -> None:
    config = init(
        prog="kvmd-vnc",
        description="VNC to KVMD proxy",
        argv=argv,
    )[2].vnc

    user_agent = htclient.make_user_agent("KVMD-VNC")

    VncServer(
        host=config.server.host,
        port=config.server.port,
        max_clients=config.server.max_clients,

        no_delay=config.server.no_delay,

        tls_ciphers=config.server.tls.ciphers,
        tls_timeout=config.server.tls.timeout,

        desired_fps=config.desired_fps,
        keymap_path=config.keymap,

        kvmd=KvmdClient(
            user_agent=user_agent,
            **config.kvmd._unpack(),
        ),
        streamer=StreamerClient(
            user_agent=user_agent,
            **config.streamer._unpack(),
        ),
        vnc_auth_manager=VncAuthManager(**config.auth.vncauth._unpack()),

        **config.server.keepalive._unpack(),
    ).run()
