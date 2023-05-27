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


from ...logging import get_logger

from .. import init

from .server import PstServer


# =====
def main(argv: (list[str] | None)=None) -> None:
    config = init(
        prog="kvmd-pst",
        description="The KVMD persistent storage manager",
        argv=argv,
        check_run=True,
    )[2]

    PstServer(
        **config.pst._unpack(ignore="server"),
    ).run(**config.pst.server._unpack())

    get_logger(0).info("Bye-bye")
