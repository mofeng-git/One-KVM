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


import asyncio

from typing import Any
from typing import Dict
from typing import List

from aiohttp.web import Request
from aiohttp.web import Response

from ....validators import check_string_in_list
from ....validators.basic import valid_string_list

from ..info import InfoManager

from ..http import exposed_http
from ..http import make_json_response
from ..http import make_text_response


# ====
def _build_metrics(metrics: List[str], name: str, value: Any) -> None:
    if isinstance(value, bool):
        value = 1 if value else 0
    if isinstance(value, (int, float)):
        metrics.append(f"# TYPE {name} gauge")
        metrics.append(f"{name} {value}")
    elif isinstance(value, dict):
        for key, val in value.items():
            if key == "parsed_flags":
                _build_metrics(metrics, name, val)
            else:
                _build_metrics(metrics, f"{name}_{key}", val)


# =====
class InfoApi:
    def __init__(self, info_manager: InfoManager) -> None:
        self.__info_manager = info_manager

    # =====

    @exposed_http("GET", "/info")
    async def __common_state_handler(self, request: Request) -> Response:
        fields = self.__valid_info_fields(request)
        results = dict(zip(fields, await asyncio.gather(*[
            self.__info_manager.get_submanager(field).get_state()
            for field in fields
        ])))
        return make_json_response(results)

    def __valid_info_fields(self, request: Request) -> List[str]:
        subs = self.__info_manager.get_subs()
        return (sorted(set(valid_string_list(
            arg=request.query.get("fields", ",".join(subs)),
            subval=(lambda field: check_string_in_list(field, "info field", subs)),
            name="info fields list",
        ))) or subs)

    @exposed_http("GET", "/export/prometheus/metrics", False)
    async def __metrics_handler(self, _: Request) -> Response:
        data = await asyncio.gather(self.__info_manager.get_submanager("hw").get_state())
        if data is None:
            return make_text_response("error", 500)
        else:
            data_exists: Dict[Any, Any] = data
            health = data_exists[0]["health"]

            metrics: List[str] = []
            _build_metrics(metrics, "pikvm", health)

            return make_text_response("\n".join(metrics))
