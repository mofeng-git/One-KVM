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

from ... import htclient

from .. import init

from .server import LocalHidServer


# =====
def main(argv: (list[str] | None)=None) -> None:
    config = init(
        prog="kvmd-localhid",
        description=" Local HID to KVMD proxy",
        check_run=True,
        argv=argv,
    )[2].localhid

    user_agent = htclient.make_user_agent("KVMD-LocalHID")

    LocalHidServer(
        kvmd=KvmdClient(user_agent=user_agent, **config.kvmd._unpack()),
    ).run()
