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


import sys
import asyncio
import threading

from typing import Tuple
from typing import Dict
from typing import Optional

import aiohttp

from pyghmi.ipmi.private.session import Session as IpmiSession
from pyghmi.ipmi.private.serversession import IpmiServer as BaseIpmiServer
from pyghmi.ipmi.private.serversession import ServerSession as IpmiServerSession

from ...logging import get_logger

from ... import __version__

from .auth import IpmiAuthManager


# =====
class IpmiServer(BaseIpmiServer):  # pylint: disable=too-many-instance-attributes,abstract-method
    # https://www.intel.com/content/dam/www/public/us/en/documents/product-briefs/ipmi-second-gen-interface-spec-v2-rev1-1.pdf
    # https://www.thomas-krenn.com/en/wiki/IPMI_Basics

    def __init__(
        self,
        auth_manager: IpmiAuthManager,

        host: str,
        port: str,
        timeout: float,

        kvmd_host: str,
        kvmd_port: int,
        kvmd_unix_path: str,
        kvmd_timeout: float,
    ) -> None:

        super().__init__(authdata=auth_manager, address=host, port=port)

        self.__auth_manager = auth_manager

        self.__host = host
        self.__port = port
        self.__timeout = timeout

        self.__kvmd_host = kvmd_host
        self.__kvmd_port = kvmd_port
        self.__kvmd_unix_path = kvmd_unix_path
        self.__kvmd_timeout = kvmd_timeout

    def run(self) -> None:
        logger = get_logger(0)
        logger.info("Listening IPMI on UPD [%s]:%d ...", self.__host, self.__port)
        try:
            while True:
                IpmiSession.wait_for_rsp(self.__timeout)
        except (SystemExit, KeyboardInterrupt):
            pass
        logger.info("Bye-bye")

    # =====

    def handle_raw_request(self, request: Dict, session: IpmiServerSession) -> None:
        handler = {
            (6, 1): lambda _, session: self.send_device_id(session),  # Get device ID
            (0, 1): self.__get_chassis_status_handler,  # Get chassis status
            (0, 2): self.__chassis_control_handler,  # Chassis control
        }.get((request["netfn"], request["command"]))
        if handler is not None:
            try:
                handler(request, session)
            except (aiohttp.ClientError, asyncio.TimeoutError):
                session.send_ipmi_response(code=0xFF)
            except Exception:
                get_logger(0).exception("Unexpected exception while handling IPMI request: netfn=%d; command=%d",
                                        request["netfn"], request["command"])
                session.send_ipmi_response(code=0xFF)
        else:
            session.send_ipmi_response(code=0xC1)

    def __get_chassis_status_handler(self, _: Dict, session: IpmiServerSession) -> None:
        result = self.__make_request("GET", "/atx", session)[1]
        data = [int(result["leds"]["power"]), 0, 0]
        session.send_ipmi_response(data=data)

    def __chassis_control_handler(self, request: Dict, session: IpmiServerSession) -> None:
        handle = {
            0: "/atx/power?action=off_hard",
            1: "/atx/power?action=on",
            3: "/atx/power?action=reset_hard",
            5: "/atx/power?action=off",
        }.get(request["data"][0], "")
        if handle:
            if self.__make_request("POST", handle, session)[0] == 409:
                code = 0xC0  # Try again later
            else:
                code = 0
        else:
            code = 0xCC  # Invalid request
        session.send_ipmi_response(code=code)

    # =====

    def __make_request(self, method: str, handle: str, ipmi_session: IpmiServerSession) -> Tuple[int, Dict]:
        result: Optional[Tuple[int, Dict]] = None
        exc_info = None

        def make_request() -> None:
            nonlocal result
            nonlocal exc_info

            loop = asyncio.new_event_loop()
            try:
                result = loop.run_until_complete(self.__make_request_async(method, handle, ipmi_session))
            except:  # noqa: E722  # pylint: disable=bare-except
                exc_info = sys.exc_info()
            finally:
                loop.close()

        thread = threading.Thread(target=make_request, daemon=True)
        thread.start()
        thread.join()
        if exc_info is not None:
            raise exc_info[1].with_traceback(exc_info[2])  # type: ignore  # pylint: disable=unsubscriptable-object
        assert result is not None
        # Dirty pylint hack
        return (result[0], result[1])  # pylint: disable=unsubscriptable-object

    async def __make_request_async(self, method: str, handle: str, ipmi_session: IpmiServerSession) -> Tuple[int, Dict]:
        logger = get_logger(0)

        assert handle.startswith("/")
        url = f"http://{self.__kvmd_host}:{self.__kvmd_port}{handle}"

        credentials = self.__auth_manager.get_credentials(ipmi_session.username.decode())
        logger.info("Performing %r request to %r from user %r (IPMI) as %r (KVMD)",
                    method, url, credentials.ipmi_user, credentials.kvmd_user)

        async with self.__make_http_session_async() as http_session:
            try:
                async with http_session.request(
                    method=method,
                    url=url,
                    headers={
                        "X-KVMD-User": credentials.kvmd_user,
                        "X-KVMD-Passwd": credentials.kvmd_passwd,
                        "User-Agent": f"KVMD-IPMI/{__version__}",
                    },
                    timeout=self.__kvmd_timeout,
                ) as response:
                    if response.status != 409:
                        response.raise_for_status()
                    return (response.status, (await response.json())["result"])
            except (aiohttp.ClientError, asyncio.TimeoutError) as err:
                logger.error("Can't perform %r request to %r: %s: %s", method, url, type(err).__name__, str(err))
                raise
            except Exception:
                logger.exception("Unexpected exception while performing %r request to %r", method, url)
                raise

    def __make_http_session_async(self) -> aiohttp.ClientSession:
        if self.__kvmd_unix_path:
            return aiohttp.ClientSession(connector=aiohttp.UnixConnector(path=self.__kvmd_unix_path))
        else:
            return aiohttp.ClientSession()
