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


import aiohttp

from . import __version__


# =====
def make_user_agent(app: str) -> str:
    return f"{app}/{__version__}"


def raise_not_200(response: aiohttp.ClientResponse) -> None:
    if response.status != 200:
        assert response.reason is not None
        response.release()
        raise aiohttp.ClientResponseError(
            response.request_info,
            response.history,
            status=response.status,
            message=response.reason,
            headers=response.headers,
        )
