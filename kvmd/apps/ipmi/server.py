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
import functools

from typing import Dict

import aiohttp

from pyghmi.ipmi.private.session import Session as IpmiSession
from pyghmi.ipmi.private.serversession import IpmiServer as BaseIpmiServer
from pyghmi.ipmi.private.serversession import ServerSession as IpmiServerSession

from ...logging import get_logger

from ...clients.kvmd import KvmdClient

from ... import aiotools

from .auth import IpmiAuthManager


# =====
class IpmiServer(BaseIpmiServer):  # pylint: disable=too-many-instance-attributes,abstract-method
    # https://www.intel.com/content/dam/www/public/us/en/documents/product-briefs/ipmi-second-gen-interface-spec-v2-rev1-1.pdf
    # https://www.thomas-krenn.com/en/wiki/IPMI_Basics

    def __init__(
        self,
        auth_manager: IpmiAuthManager,
        kvmd: KvmdClient,

        host: str,
        port: str,
        timeout: float,
    ) -> None:

        super().__init__(authdata=auth_manager, address=host, port=port)

        self.__auth_manager = auth_manager
        self.__kvmd = kvmd

        self.__host = host
        self.__port = port
        self.__timeout = timeout

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
        result = self.__make_request(session, "atx.get_state()", "atx.get_state")
        data = [int(result["leds"]["power"]), 0, 0]
        session.send_ipmi_response(data=data)

    def __chassis_control_handler(self, request: Dict, session: IpmiServerSession) -> None:
        action = {
            0: "off_hard",
            1: "on",
            3: "reset_hard",
            5: "off",
        }.get(request["data"][0], "")
        if action:
            if not self.__make_request(session, f"atx.switch_power({action})", "atx.switch_power", action=action):
                code = 0xC0  # Try again later
            else:
                code = 0
        else:
            code = 0xCC  # Invalid request
        session.send_ipmi_response(code=code)

    # =====

    def __make_request(self, session: IpmiServerSession, name: str, method_path: str, **kwargs):  # type: ignore
        async def runner():  # type: ignore
            logger = get_logger(0)
            credentials = self.__auth_manager.get_credentials(session.username.decode())
            logger.info("Performing request %s from user %r (IPMI) as %r (KVMD)",
                        name, credentials.ipmi_user, credentials.kvmd_user)
            try:
                async with self.__kvmd.make_session(credentials.kvmd_user, credentials.kvmd_passwd) as kvmd_session:
                    method = functools.reduce(getattr, method_path.split("."), kvmd_session)
                    return (await method(**kwargs))
            except (aiohttp.ClientError, asyncio.TimeoutError) as err:
                logger.error("Can't perform request %s: %s", name, str(err))
                raise

        return aiotools.run_sync(runner())
