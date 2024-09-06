# ========================================================================== #
#                                                                            #
#    KVMD - The main PiKVM daemon.                                           #
#                                                                            #
#    Copyright (C) 2020  Maxim Devaev <mdevaev@gmail.com>                    #
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
import contextlib
import struct
import types

from typing import Callable
from typing import AsyncGenerator

import aiohttp

from .. import aiotools
from .. import htclient
from .. import htserver


# =====
class _BaseApiPart:
    def __init__(
        self,
        ensure_http_session: Callable[[], aiohttp.ClientSession],
        make_url: Callable[[str], str],
    ) -> None:

        self._ensure_http_session = ensure_http_session
        self._make_url = make_url

    async def _set_params(self, handle: str, **params: (int | str | None)) -> None:
        session = self._ensure_http_session()
        async with session.post(
            url=self._make_url(handle),
            params={
                key: value
                for (key, value) in params.items()
                if value is not None
            },
        ) as response:
            htclient.raise_not_200(response)


class _AuthApiPart(_BaseApiPart):
    async def check(self) -> bool:
        session = self._ensure_http_session()
        try:
            async with session.get(self._make_url("auth/check")) as response:
                htclient.raise_not_200(response)
                return True
        except aiohttp.ClientResponseError as err:
            if err.status in [400, 401, 403]:
                return False
            raise


class _StreamerApiPart(_BaseApiPart):
    async def get_state(self) -> dict:
        session = self._ensure_http_session()
        async with session.get(self._make_url("streamer")) as response:
            htclient.raise_not_200(response)
            return (await response.json())["result"]

    async def set_params(self, quality: (int | None)=None, desired_fps: (int | None)=None) -> None:
        await self._set_params(
            "streamer/set_params",
            quality=quality,
            desired_fps=desired_fps,
        )


class _HidApiPart(_BaseApiPart):
    async def get_keymaps(self) -> tuple[str, set[str]]:
        session = self._ensure_http_session()
        async with session.get(self._make_url("hid/keymaps")) as response:
            htclient.raise_not_200(response)
            result = (await response.json())["result"]
            return (result["keymaps"]["default"], set(result["keymaps"]["available"]))

    async def print(self, text: str, limit: int, keymap_name: str) -> None:
        session = self._ensure_http_session()
        async with session.post(
            url=self._make_url("hid/print"),
            params={"limit": limit, "keymap": keymap_name},
            data=text,
        ) as response:
            htclient.raise_not_200(response)

    async def set_params(self, keyboard_output: (str | None)=None, mouse_output: (str | None)=None) -> None:
        await self._set_params(
            "hid/set_params",
            keyboard_output=keyboard_output,
            mouse_output=mouse_output,
        )


class _AtxApiPart(_BaseApiPart):
    async def get_state(self) -> dict:
        session = self._ensure_http_session()
        async with session.get(self._make_url("atx")) as response:
            htclient.raise_not_200(response)
            return (await response.json())["result"]

    async def switch_power(self, action: str) -> bool:
        session = self._ensure_http_session()
        try:
            async with session.post(
                url=self._make_url("atx/power"),
                params={"action": action},
            ) as response:
                htclient.raise_not_200(response)
                return True
        except aiohttp.ClientResponseError as err:
            if err.status == 409:
                return False
            raise


# =====
class KvmdClientWs:
    def __init__(self, ws: aiohttp.ClientWebSocketResponse) -> None:
        self.__ws = ws

        self.__writer_queue: "asyncio.Queue[tuple[str, dict] | bytes]" = asyncio.Queue()
        self.__communicated = False

    async def communicate(self) -> AsyncGenerator[tuple[str, dict], None]:  # pylint: disable=too-many-branches
        assert not self.__communicated
        self.__communicated = True
        receive_task: (asyncio.Task | None) = None
        writer_task: (asyncio.Task | None) = None
        try:
            while True:
                if receive_task is None:
                    receive_task = asyncio.create_task(self.__ws.receive())
                if writer_task is None:
                    writer_task = asyncio.create_task(self.__writer_queue.get())

                done = (await aiotools.wait_first(receive_task, writer_task))[0]

                if receive_task in done:
                    msg = receive_task.result()
                    if msg.type == aiohttp.WSMsgType.TEXT:
                        yield htserver.parse_ws_event(msg.data)
                    elif msg.type == aiohttp.WSMsgType.CLOSE:
                        await self.__ws.close()
                    elif msg.type == aiohttp.WSMsgType.CLOSED:
                        break
                    else:
                        raise RuntimeError(f"Unhandled WS message type: {msg!r}")
                    receive_task = None

                if writer_task in done:
                    payload = writer_task.result()
                    if isinstance(payload, bytes):
                        await self.__ws.send_bytes(payload)
                    else:
                        await htserver.send_ws_event(self.__ws, *payload)
                    writer_task = None
        finally:
            if receive_task:
                receive_task.cancel()
            if writer_task:
                writer_task.cancel()
            try:
                await aiotools.shield_fg(self.__ws.close())
            except Exception:
                pass
            finally:
                self.__communicated = False

    async def send_key_event(self, key: str, state: bool) -> None:
        await self.__writer_queue.put(bytes([1, state]) + key.encode("ascii"))

    async def send_mouse_button_event(self, button: str, state: bool) -> None:
        await self.__writer_queue.put(bytes([2, state]) + button.encode("ascii"))

    async def send_mouse_move_event(self, to_x: int, to_y: int) -> None:
        await self.__writer_queue.put(struct.pack(">bhh", 3, to_x, to_y))

    async def send_mouse_wheel_event(self, delta_x: int, delta_y: int) -> None:
        await self.__writer_queue.put(struct.pack(">bbbb", 5, 0, delta_x, delta_y))


class KvmdClientSession:
    def __init__(
        self,
        make_http_session: Callable[[], aiohttp.ClientSession],
        make_url: Callable[[str], str],
    ) -> None:

        self.__make_http_session = make_http_session
        self.__make_url = make_url

        self.__http_session: (aiohttp.ClientSession | None) = None

        args = (self.__ensure_http_session, make_url)

        self.auth = _AuthApiPart(*args)
        self.streamer = _StreamerApiPart(*args)
        self.hid = _HidApiPart(*args)
        self.atx = _AtxApiPart(*args)

    @contextlib.asynccontextmanager
    async def ws(self) -> AsyncGenerator[KvmdClientWs, None]:
        session = self.__ensure_http_session()
        async with session.ws_connect(self.__make_url("ws")) as ws:
            yield KvmdClientWs(ws)

    def __ensure_http_session(self) -> aiohttp.ClientSession:
        if not self.__http_session:
            self.__http_session = self.__make_http_session()
        return self.__http_session

    async def close(self) -> None:
        if self.__http_session:
            await self.__http_session.close()
            self.__http_session = None

    async def __aenter__(self) -> "KvmdClientSession":
        return self

    async def __aexit__(
        self,
        _exc_type: type[BaseException],
        _exc: BaseException,
        _tb: types.TracebackType,
    ) -> None:

        await self.close()


class KvmdClient:
    def __init__(
        self,
        unix_path: str,
        timeout: float,
        user_agent: str,
    ) -> None:

        self.__unix_path = unix_path
        self.__timeout = timeout
        self.__user_agent = user_agent

    def make_session(self, user: str, passwd: str) -> KvmdClientSession:
        return KvmdClientSession(
            make_http_session=(lambda: self.__make_http_session(user, passwd)),
            make_url=self.__make_url,
        )

    def __make_http_session(self, user: str, passwd: str) -> aiohttp.ClientSession:
        kwargs: dict = {
            "headers": {
                "X-KVMD-User": user,
                "X-KVMD-Passwd": passwd,
                "User-Agent": self.__user_agent,
            },
            "connector": aiohttp.UnixConnector(path=self.__unix_path),
            "timeout": aiohttp.ClientTimeout(total=self.__timeout),
        }
        return aiohttp.ClientSession(**kwargs)

    def __make_url(self, handle: str) -> str:
        assert not handle.startswith("/"), handle
        return f"http://localhost:0/{handle}"
