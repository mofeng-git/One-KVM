# ========================================================================== #
#                                                                            #
#    KVMD - The main PiKVM daemon.                                           #
#                                                                            #
#    Copyright (C) 2018-2024  Maxim Devaev <mdevaev@gmail.com>               #
#    Copyright (C) 2023-2025  SilentWind <mofeng654321@hotmail.com>          #
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

from aiohttp.web import Request
from aiohttp.web import StreamResponse

from ....errors import OperationError

from ....htserver import exposed_http
from ....htserver import start_streaming

from ....validators.basic import valid_bool
from ....validators.kvm import valid_log_seek

from ..logreader import LogReader


# =====
class LogReaderDisabledError(OperationError):
    def __init__(self) -> None:
        super().__init__("LogReader is disabled")


class LogApi:
    def __init__(self, log_reader: (LogReader | None)) -> None:
        self.__log_reader = log_reader

    # =====

    @exposed_http("GET", "/log")
    async def __log_handler(self, req: Request) -> StreamResponse:
        if self.__log_reader is None:
            raise LogReaderDisabledError()
        seek = valid_log_seek(req.query.get("seek", 0))
        follow = valid_bool(req.query.get("follow", False))
        resp = await start_streaming(req, "text/plain")
        try:
            async for record in self.__log_reader.poll_log(seek, follow):
                await resp.write(("[%s %s] --- %s" % (
                    record["dt"].strftime("%Y-%m-%d %H:%M:%S"),
                    record["service"],
                    record["msg"],
                )).encode("utf-8") + b"\r\n")
        except Exception as exception:
            await resp.write(f"Module systemd.journal is unavailable.\n{exception}".encode("utf-8"))
            return resp
        return resp
