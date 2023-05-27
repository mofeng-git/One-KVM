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


import socket
import select
import multiprocessing
import multiprocessing.synchronize
import dataclasses
import contextlib
import queue

from typing import Literal
from typing import Generator

from ....logging import get_logger

from .... import tools
from .... import aiomulti

from ....keyboard.mappings import UsbKey

from ..otg.events import BaseEvent
from ..otg.events import ClearEvent
from ..otg.events import ResetEvent

from ..otg.events import get_led_caps
from ..otg.events import get_led_scroll
from ..otg.events import get_led_num

from ..otg.events import MouseButtonEvent
from ..otg.events import MouseRelativeEvent
from ..otg.events import MouseWheelEvent
from ..otg.events import make_mouse_report

from ..otg.events import KeyEvent
from ..otg.events import ModifierEvent
from ..otg.events import make_keyboard_report

from .bluez import HID_CTL_PORT
from .bluez import HID_INT_PORT
from .bluez import BluezIface


# =====
_RoleT = Literal["CTL", "INT"]
_SockAttrT = Literal["ctl_sock", "int_sock"]


@dataclasses.dataclass
class _BtClient:
    addr: str
    ctl_sock: (socket.socket | None) = None
    int_sock: (socket.socket | None) = None


# =====
class BtServer:  # pylint: disable=too-many-instance-attributes
    def __init__(
        self,
        iface: BluezIface,

        control_public: bool,
        unpair_on_close: bool,

        max_clients: int,
        socket_timeout: float,
        select_timeout: float,

        notifier: aiomulti.AioProcessNotifier,
        stop_event: multiprocessing.synchronize.Event,
    ) -> None:

        self.__iface = iface

        self.__control_public = control_public
        self.__unpair_on_close = unpair_on_close

        self.__max_clients = max_clients
        self.__socket_timeout = socket_timeout
        self.__select_timeout = select_timeout

        self.__stop_event = stop_event

        self.__clients: dict[str, _BtClient] = {}
        self.__to_read: set[socket.socket] = set()

        self.__events_queue: "multiprocessing.Queue[BaseEvent]" = multiprocessing.Queue()

        self.__state_flags = aiomulti.AioSharedFlags({
            "online": False,
            "caps": False,
            "scroll": False,
            "num": False,
        }, notifier)
        self.__modifiers: set[UsbKey] = set()
        self.__keys: list[UsbKey | None] = [None] * 6
        self.__mouse_buttons = 0

    def run(self) -> None:
        with self.__iface:
            self.__iface.configure()
            self.__set_public(True)
            addr = self.__iface.get_address()
            try:
                with self.__listen("CTL", addr, HID_CTL_PORT) as ctl_sock:
                    with self.__listen("INT", addr, HID_INT_PORT) as int_sock:
                        self.__main_loop(ctl_sock, int_sock)
            finally:
                self.__close_all_clients(no_change_public=True)
                self.__set_public(False)

    async def get_state(self) -> dict:
        return (await self.__state_flags.get())

    def queue_event(self, event: BaseEvent) -> None:
        if not self.__stop_event.is_set():
            self.__events_queue.put_nowait(event)

    def clear_events(self) -> None:
        # FIXME: Если очистка производится со стороны процесса хида, то возможна гонка между
        # очисткой и добавлением события ClearEvent. Неприятно, но не смертельно.
        # Починить блокировкой после перехода на асинхронные очереди.
        tools.clear_queue(self.__events_queue)
        self.queue_event(ClearEvent())

    # =====

    @contextlib.contextmanager
    def __listen(self, role: _RoleT, addr: str, port: int) -> Generator[socket.socket, None, None]:
        get_logger(0).info("Listening [%s]:%d for %s ...", addr, port, role)
        with socket.socket(socket.AF_BLUETOOTH, socket.SOCK_SEQPACKET, socket.BTPROTO_L2CAP) as sock:  # type: ignore
            sock.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
            sock.settimeout(self.__socket_timeout)
            sock.bind((addr, port))
            sock.listen(5)
            yield sock

    def __main_loop(  # pylint: disable=too-many-branches
        self,
        server_ctl_sock: socket.socket,
        server_int_sock: socket.socket,
    ) -> None:

        qr = self.__events_queue._reader  # type: ignore  # pylint: disable=protected-access
        self.__to_read = set([qr, server_ctl_sock, server_int_sock])
        self.__clients = {}

        while not self.__stop_event.is_set():
            (ready_read, _, _) = select.select(self.__to_read, [], [], self.__select_timeout)

            if server_ctl_sock in ready_read:
                self.__accept_client("CTL", server_ctl_sock, "ctl_sock")
            if server_int_sock in ready_read:
                self.__accept_client("INT", server_int_sock, "int_sock")

            for client in list(self.__clients.values()):
                sock = client.ctl_sock
                if sock in ready_read:
                    assert sock is not None
                    try:
                        data = sock.recv(1024)
                        if not data:
                            self.__close_client("CTL", client, "ctl_sock")
                        elif data == b"\x71":
                            sock.send(b"\x00")
                    except Exception as err:
                        get_logger(0).exception("CTL socket error on %s: %s", client.addr, tools.efmt(err))
                        self.__close_client("CTL", client, "ctl_sock")
                        continue

                sock = client.int_sock
                if sock in ready_read:
                    assert sock is not None
                    try:
                        data = sock.recv(1024)
                        if not data:
                            self.__close_client("INT", client, "int_sock")
                        elif data[:2] == b"\xA2\x01":
                            self.__process_leds(data[2])
                    except Exception as err:
                        get_logger(0).exception("INT socket error on %s: %s", client.addr, tools.efmt(err))
                        self.__close_client("INT", client, "ctl_sock")

            if qr in ready_read:
                self.__process_events()

    # =====

    def __process_leds(self, leds: int) -> None:
        self.__state_flags.update(
            caps=get_led_caps(leds),
            scroll=get_led_scroll(leds),
            num=get_led_num(leds),
        )

    def __process_events(self) -> None:  # pylint: disable=too-many-branches
        for _ in range(self.__events_queue.qsize()):
            try:
                event = self.__events_queue.get_nowait()
            except queue.Empty:
                break
            else:
                if isinstance(event, ResetEvent):
                    self.__close_all_clients()
                    return

                elif isinstance(event, ClearEvent):
                    self.__clear_modifiers()
                    self.__clear_keys()
                    self.__mouse_buttons = 0
                    self.__send_keyboard_state()
                    self.__send_mouse_state(0, 0, 0)

                elif isinstance(event, ModifierEvent):
                    if event.modifier in self.__modifiers:  # Ранее нажатый модификатор отжимаем
                        self.__modifiers.remove(event.modifier)
                        self.__send_keyboard_state()
                    if event.state:  # Нажимаем если нужно
                        self.__modifiers.add(event.modifier)
                        self.__send_keyboard_state()

                elif isinstance(event, KeyEvent):
                    if event.key in self.__keys:  # Ранее нажатую клавишу отжимаем
                        self.__keys[self.__keys.index(event.key)] = None
                        self.__send_keyboard_state()
                    elif event.state and None not in self.__keys:  # Если слоты полны - отжимаем всё
                        self.__clear_keys()
                        self.__send_keyboard_state()
                    if event.state:  # Нажимаем если нужно
                        self.__keys[self.__keys.index(None)] = event.key
                        self.__send_keyboard_state()

                elif isinstance(event, MouseButtonEvent):
                    if event.code & self.__mouse_buttons:  # Ранее нажатую кнопку отжимаем
                        self.__mouse_buttons &= ~event.code
                        self.__send_mouse_state(0, 0, 0)
                    if event.state:  # Нажимаем если нужно
                        self.__mouse_buttons |= event.code
                        self.__send_mouse_state(0, 0, 0)

                elif isinstance(event, MouseRelativeEvent):
                    self.__send_mouse_state(event.delta_x, event.delta_y, 0)

                elif isinstance(event, MouseWheelEvent):
                    self.__send_mouse_state(0, 0, event.delta_y)

    def __send_keyboard_state(self) -> None:
        for client in list(self.__clients.values()):
            if client.int_sock is not None:
                report = make_keyboard_report(self.__modifiers, self.__keys)
                self.__send_report(client, "keyboard", b"\xA1\x01" + report)

    def __send_mouse_state(self, move_x: int, move_y: int, wheel_y: int) -> None:
        for client in list(self.__clients.values()):
            if client.int_sock is not None:
                report = make_mouse_report(False, self.__mouse_buttons, move_x, move_y, None, wheel_y)
                self.__send_report(client, "mouse", b"\xA1\x02" + report)

    def __send_report(self, client: _BtClient, name: str, report: bytes) -> None:
        assert client.int_sock is not None
        try:
            client.int_sock.send(report)
        except Exception as err:
            get_logger(0).info("Can't send %s report to %s: %s", name, client.addr, tools.efmt(err))
            self.__close_client_pair(client)

    def __clear_modifiers(self) -> None:
        self.__modifiers.clear()

    def __clear_keys(self) -> None:
        self.__keys = [None] * 6

    def __clear_state(self) -> None:
        self.__state_flags.update(
            online=False,
            caps=False,
            scroll=False,
            num=False,
        )
        self.__clear_modifiers()
        self.__clear_keys()
        self.__mouse_buttons = 0

    # =====

    def __accept_client(self, role: _RoleT, server_sock: socket.socket, sock_attr: _SockAttrT) -> None:
        try:
            (sock, peer) = server_sock.accept()
            sock.setblocking(True)
        except Exception:
            get_logger(0).exception("Can't accept %s client", role)
        else:
            if peer[0] not in self.__clients:
                if len(self.__clients) >= self.__max_clients:
                    self.__close_sock(sock)
                    get_logger(0).info("Refused %s client: %s: max clients reached", role, peer[0])
                    return
                self.__clients[peer[0]] = _BtClient(peer[0])
            client = self.__clients[peer[0]]

            assert hasattr(client, sock_attr)
            setattr(client, sock_attr, sock)
            self.__to_read.add(sock)

            get_logger(0).info("Accepted %s client: %s", role, peer[0])
            self.__state_flags.update(online=True)

            self.__set_public(len(self.__clients) < self.__max_clients)

    def __close_client(self, role: _RoleT, client: _BtClient, sock_attr: _SockAttrT, no_change_public: bool=False) -> None:
        sock = getattr(client, sock_attr)
        if sock is not None:
            self.__close_sock(sock)
            setattr(client, sock_attr, None)
            self.__to_read.remove(sock)

        get_logger(0).info("Closed %s client %s", role, client.addr)

        if client.ctl_sock is None and client.int_sock is None:
            self.__clients.pop(client.addr)
            if self.__unpair_on_close:
                self.__unpair_client(client)

        if len(self.__clients) == 0:
            self.__clear_state()

        if not no_change_public:
            self.__set_public(len(self.__clients) < self.__max_clients)

    def __close_client_pair(self, client: _BtClient, no_change_public: bool=False) -> None:
        self.__close_client("CTL", client, "ctl_sock", no_change_public)
        self.__close_client("INT", client, "int_sock", no_change_public)

    def __close_all_clients(self, no_change_public: bool=False) -> None:
        for client in list(self.__clients.values()):
            self.__close_client_pair(client, no_change_public)
        self.__clear_state()
        if not no_change_public:
            self.__set_public(True)

    def __close_sock(self, sock: socket.socket) -> None:
        try:
            sock.close()
        except Exception:
            pass

    # =====

    def __set_public(self, public: bool) -> None:
        logger = get_logger(0)
        if self.__control_public:
            logger.info("Publishing ..." if public else "Unpublishing ...")
            try:
                self.__iface.set_public(public)
            except Exception as err:
                logger.error("Can't change public mode: %s", tools.efmt(err))

    def __unpair_client(self, client: _BtClient) -> None:
        logger = get_logger(0)
        logger.info("Unpairing %s ...", client.addr)
        try:
            self.__iface.unpair(client.addr)
        except Exception as err:
            logger.error("Can't unpair %s: %s", client.addr, tools.efmt(err))
