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
import select
import asyncio
import threading
import multiprocessing
import functools
import queue

import aiohttp
import serial

from pyghmi.ipmi.console import ServerConsole as IpmiConsole
from pyghmi.ipmi.private.session import Session as IpmiSession
from pyghmi.ipmi.private.serversession import IpmiServer as BaseIpmiServer
from pyghmi.ipmi.private.serversession import ServerSession as IpmiServerSession

from ...logging import get_logger

from ...clients.kvmd import KvmdClient

from ... import aiotools
from ... import network

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
        port: int,
        timeout: float,

        sol_device_path: str,
        sol_speed: int,
        sol_select_timeout: float,
        sol_proxy_port: int,
    ) -> None:

        host = network.get_listen_host(host)

        super().__init__(authdata=auth_manager, address=host, port=port)

        self.__auth_manager = auth_manager
        self.__kvmd = kvmd

        self.__host = host
        self.__port = port
        self.__timeout = timeout

        self.__sol_device_path = sol_device_path
        self.__sol_speed = sol_speed
        self.__sol_select_timeout = sol_select_timeout
        self.__sol_proxy_port = (sol_proxy_port or port)

        self.__sol_lock = threading.Lock()
        self.__sol_console: (IpmiConsole | None) = None
        self.__sol_thread: (threading.Thread | None) = None
        self.__sol_stop = False

    def run(self) -> None:
        logger = get_logger(0)
        logger.info("Listening IPMI on UPD [%s]:%d ...", self.__host, self.__port)
        try:
            while True:
                IpmiSession.wait_for_rsp(self.__timeout)
        except (SystemExit, KeyboardInterrupt):
            pass
        self.__stop_sol_worker()
        logger.info("Bye-bye")

    # =====

    def handle_raw_request(self, request: dict, session: IpmiServerSession) -> None:
        handler = {
            (6, 1): (lambda _, session: self.send_device_id(session)),  # Get device ID
            (6, 7): self.__get_power_state_handler,  # Power state
            (6, 4): self.__get_selftest_status_handler,  # Self-test
            (6, 0x38): (lambda _, session: session.send_ipmi_response()),  # Get channel auth types
            (0, 1): self.__get_chassis_status_handler,  # Get chassis status
            (0, 2): self.__chassis_control_handler,  # Chassis control
            (6, 0x48): self.__activate_sol_handler,  # Enable SOL
            (6, 0x49): self.__deactivate_sol_handler,  # Disable SOL
        }.get((request["netfn"], request["command"]))
        if handler is not None:
            try:
                handler(request, session)
            except (aiohttp.ClientError, asyncio.TimeoutError):
                session.send_ipmi_response(code=0xFF)
            except Exception:
                get_logger(0).exception("[%s]: Unexpected exception while handling IPMI request: netfn=%d; command=%d",
                                        session.sockaddr[0], request["netfn"], request["command"])
                session.send_ipmi_response(code=0xFF)
        else:
            session.send_ipmi_response(code=0xC1)

    # =====

    def __get_power_state_handler(self, _: dict, session: IpmiServerSession) -> None:
        # https://github.com/arcress0/ipmiutil/blob/e2f6e95127d22e555f959f136d9bb9543c763896/util/ireset.c#L654
        result = self.__make_request(session, "atx.get_state() [power]", "atx.get_state")
        data = [(0 if result["leds"]["power"] else 5)]
        session.send_ipmi_response(data=data)

    def __get_selftest_status_handler(self, _: dict, session: IpmiServerSession) -> None:
        # https://github.com/arcress0/ipmiutil/blob/e2f6e95127d22e555f959f136d9bb9543c763896/util/ihealth.c#L858
        data = [0x0055]
        try:
            self.__make_request(session, "atx.get_state() [health]", "atx.get_state")
        except Exception:
            data = [0]
        session.send_ipmi_response(data=data)

    def __get_chassis_status_handler(self, _: dict, session: IpmiServerSession) -> None:
        result = self.__make_request(session, "atx.get_state() [chassis]", "atx.get_state")
        data = [int(result["leds"]["power"]), 0, 0]
        session.send_ipmi_response(data=data)

    def __chassis_control_handler(self, request: dict, session: IpmiServerSession) -> None:
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

    def __make_request(self, session: IpmiServerSession, name: str, func_path: str, **kwargs):  # type: ignore
        async def runner():  # type: ignore
            logger = get_logger(0)
            credentials = self.__auth_manager.get_credentials(session.username.decode())
            logger.info("[%s]: Performing request %s from user %r (IPMI) as %r (KVMD)",
                        session.sockaddr[0], name, credentials.ipmi_user, credentials.kvmd_user)
            try:
                async with self.__kvmd.make_session(credentials.kvmd_user, credentials.kvmd_passwd) as kvmd_session:
                    func = functools.reduce(getattr, func_path.split("."), kvmd_session)
                    return (await func(**kwargs))
            except (aiohttp.ClientError, asyncio.TimeoutError) as err:
                logger.error("[%s]: Can't perform request %s: %s", session.sockaddr[0], name, err)
                raise

        return aiotools.run_sync(runner())

    # =====

    def __activate_sol_handler(self, _: dict, session: IpmiServerSession) -> None:
        with self.__sol_lock:
            if not self.__sol_device_path:
                session.send_ipmi_response(code=0x81)  # SOL disabled
            elif not os.access(self.__sol_device_path, os.R_OK | os.W_OK):
                get_logger(0).info("Can't activate SOL because %s is unavailable", self.__sol_device_path)
                session.send_ipmi_response(code=0x81)  # SOL disabled
            elif self.__is_sol_activated():
                session.send_ipmi_response(code=0x80)  # Already activated
            else:
                get_logger(0).info("Activating SOL ...")
                self.__stop_sol_worker()  # Join if dead
                session.send_ipmi_response(data=[
                    0, 0, 0, 0, 1, 0, 1, 0,
                    (self.__sol_proxy_port >> 8 & 0xFF), (self.__sol_proxy_port & 0xFF),
                    0xFF, 0xFF,
                ])
                self.__start_sol_worker(session)

    def __deactivate_sol_handler(self, _: dict, session: IpmiServerSession) -> None:
        with self.__sol_lock:
            if not self.__sol_device_path:
                session.send_ipmi_response(code=0x81)
            elif not self.__is_sol_activated():
                session.send_ipmi_response(code=0x80)
            else:
                get_logger(0).info("Deactivating SOL ...")
                self.__stop_sol_worker()

    def __is_sol_activated(self) -> bool:
        return (self.__sol_thread is not None and self.__sol_thread.is_alive())

    def __start_sol_worker(self, session: IpmiServerSession) -> None:
        assert self.__sol_console is None
        assert self.__sol_thread is None
        user_queue: "multiprocessing.Queue[bytes]" = multiprocessing.Queue()  # Only for select()
        self.__sol_console = IpmiConsole(session, user_queue.put_nowait)
        self.__sol_thread = threading.Thread(target=self.__sol_worker, args=(user_queue,), daemon=True)
        self.__sol_thread.start()

    def __stop_sol_worker(self) -> None:
        if self.__sol_thread is not None:
            if self.__sol_thread.is_alive():
                self.__sol_stop = True
            self.__sol_thread.join()
            self.__sol_stop = False
            self.__sol_thread = None
        self.__close_sol_console()

    def __close_sol_console(self) -> None:
        if self.__sol_console is not None:
            self.__sol_console.close()
            self.__sol_console = None
            get_logger(0).info("SOL closed")

    def __sol_worker(self, user_queue: "multiprocessing.Queue[bytes]") -> None:
        logger = get_logger(0)
        logger.info("Starting SOL worker ...")
        try:
            assert self.__sol_console is not None
            with serial.Serial(self.__sol_device_path, self.__sol_speed) as tty:
                logger.info("Opened SOL port %s at speed=%d", self.__sol_device_path, self.__sol_speed)
                qr = user_queue._reader  # type: ignore  # pylint: disable=protected-access
                try:
                    while not self.__sol_stop:
                        ready = select.select([qr, tty], [], [], self.__sol_select_timeout)[0]
                        if qr in ready:
                            data = b""
                            for _ in range(user_queue.qsize()):  # Don't hold on this with [not empty()]
                                try:
                                    data += user_queue.get_nowait()
                                except queue.Empty:
                                    break
                            if data:
                                tty.write(data)
                        if tty in ready:
                            self.__sol_console.send_data(tty.read_all())
                finally:
                    logger.info("Closed SOL port %s", self.__sol_device_path)
        except Exception:
            logger.exception("SOL worker error")
            self.__close_sol_console()
        finally:
            logger.info("SOL worker finished")
