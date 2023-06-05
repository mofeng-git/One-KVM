# ========================================================================== #
#                                                                            #
#    KVMD - The main PiKVM daemon.                                           #
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


from ...clients.kvmd import KvmdClient
from ...clients.streamer import StreamFormats
from ...clients.streamer import BaseStreamerClient
from ...clients.streamer import HttpStreamerClient
from ...clients.streamer import MemsinkStreamerClient

from ... import htclient

from .. import init

from .vncauth import VncAuthManager
from .server import VncServer


# =====
def main(argv: (list[str] | None)=None) -> None:
    config = init(
        prog="kvmd-vnc",
        description="VNC to KVMD proxy",
        check_run=True,
        argv=argv,
    )[2].vnc

    user_agent = htclient.make_user_agent("KVMD-VNC")

    def make_memsink_streamer(name: str, fmt: int) -> (MemsinkStreamerClient | None):
        if getattr(config.memsink, name).sink:
            return MemsinkStreamerClient(name.upper(), fmt, **getattr(config.memsink, name)._unpack())
        return None

    streamers: list[BaseStreamerClient] = list(filter(None, [
        make_memsink_streamer("h264", StreamFormats.H264),
        make_memsink_streamer("jpeg", StreamFormats.JPEG),
        HttpStreamerClient(name="JPEG", user_agent=user_agent, **config.streamer._unpack()),
    ]))

    VncServer(
        host=config.server.host,
        port=config.server.port,
        max_clients=config.server.max_clients,

        no_delay=config.server.no_delay,

        tls_ciphers=config.server.tls.ciphers,
        tls_timeout=config.server.tls.timeout,
        x509_cert_path=config.server.tls.x509.cert,
        x509_key_path=config.server.tls.x509.key,

        desired_fps=config.desired_fps,
        mouse_output=config.mouse_output,
        keymap_path=config.keymap,

        kvmd=KvmdClient(user_agent=user_agent, **config.kvmd._unpack()),
        streamers=streamers,
        vnc_auth_manager=VncAuthManager(**config.auth.vncauth._unpack()),

        **config.server.keepalive._unpack(),
        **config.auth.vencrypt._unpack(),
    ).run()
