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


import base64

from aiohttp.web import Request
from aiohttp.web import Response

from ....htserver import UnauthorizedError
from ....htserver import ForbiddenError
from ....htserver import HttpExposed
from ....htserver import exposed_http
from ....htserver import make_json_response
from ....htserver import set_request_auth_info

from ....validators.auth import valid_user
from ....validators.auth import valid_passwd
from ....validators.auth import valid_auth_token

from ..auth import AuthManager


# =====
_COOKIE_AUTH_TOKEN = "auth_token"


async def check_request_auth(auth_manager: AuthManager, exposed: HttpExposed, request: Request) -> None:
    if auth_manager.is_auth_required(exposed):
        user = request.headers.get("X-KVMD-User", "")
        if user:
            user = valid_user(user)
            passwd = request.headers.get("X-KVMD-Passwd", "")
            set_request_auth_info(request, f"{user} (xhdr)")
            if not (await auth_manager.authorize(user, valid_passwd(passwd))):
                raise ForbiddenError()
            return

        token = request.cookies.get(_COOKIE_AUTH_TOKEN, "")
        if token:
            user = auth_manager.check(valid_auth_token(token))  # type: ignore
            if not user:
                set_request_auth_info(request, "- (token)")
                raise ForbiddenError()
            set_request_auth_info(request, f"{user} (token)")
            return

        basic_auth = request.headers.get("Authorization", "")
        if basic_auth and basic_auth[:6].lower() == "basic ":
            try:
                (user, passwd) = base64.b64decode(basic_auth[6:]).decode("utf-8").split(":")
            except Exception:
                raise UnauthorizedError()
            user = valid_user(user)
            set_request_auth_info(request, f"{user} (basic)")
            if not (await auth_manager.authorize(user, valid_passwd(passwd))):
                raise ForbiddenError()
            return

        raise UnauthorizedError()


class AuthApi:
    def __init__(self, auth_manager: AuthManager) -> None:
        self.__auth_manager = auth_manager

    # =====

    @exposed_http("POST", "/auth/login", auth_required=False)
    async def __login_handler(self, request: Request) -> Response:
        if self.__auth_manager.is_auth_enabled():
            credentials = await request.post()
            token = await self.__auth_manager.login(
                user=valid_user(credentials.get("user", "")),
                passwd=valid_passwd(credentials.get("passwd", "")),
            )
            if token:
                return make_json_response(set_cookies={_COOKIE_AUTH_TOKEN: token})
            raise ForbiddenError()
        return make_json_response()

    @exposed_http("POST", "/auth/logout")
    async def __logout_handler(self, request: Request) -> Response:
        if self.__auth_manager.is_auth_enabled():
            token = valid_auth_token(request.cookies.get(_COOKIE_AUTH_TOKEN, ""))
            self.__auth_manager.logout(token)
        return make_json_response()

    @exposed_http("GET", "/auth/check")
    async def __check_handler(self, _: Request) -> Response:
        return make_json_response()
