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


import os
import contextlib

from typing import AsyncGenerator

import aiohttp
import aiohttp.multipart

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


def get_filename(response: aiohttp.ClientResponse) -> str:
    try:
        disp = response.headers["Content-Disposition"]
        parsed = aiohttp.multipart.parse_content_disposition(disp)
        return str(parsed[1]["filename"])
    except Exception:
        try:
            return os.path.basename(response.url.path)
        except Exception:
            raise aiohttp.ClientError("Can't determine filename")


@contextlib.asynccontextmanager
async def download(
    url: str,
    verify: bool=True,
    timeout: float=10.0,
    read_timeout: (float | None)=None,
    app: str="KVMD",
) -> AsyncGenerator[aiohttp.ClientResponse, None]:

    kwargs: dict = {
        "headers": {"User-Agent": make_user_agent(app)},
        "timeout": aiohttp.ClientTimeout(
            connect=timeout,
            sock_connect=timeout,
            sock_read=(read_timeout if read_timeout is not None else timeout),
        ),
    }
    async with aiohttp.ClientSession(**kwargs) as session:
        async with session.get(url, verify_ssl=verify) as response:
            raise_not_200(response)
            yield response
