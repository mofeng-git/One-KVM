import asyncio

from ...logging import get_logger

from ... import gpio

from .. import init

from .auth import AuthManager
from .info import InfoManager
from .logreader import LogReader
from .hid import Hid
from .atx import Atx
from .msd import MassStorageDevice
from .streamer import Streamer
from .server import Server


# =====
def main() -> None:
    config = init("kvmd", description="The main Pi-KVM daemon")[2].kvmd
    with gpio.bcm():
        # pylint: disable=protected-access
        loop = asyncio.get_event_loop()
        Server(
            auth_manager=AuthManager(**config.auth._unpack_renamed()),
            info_manager=InfoManager(loop=loop, **config.info._unpack_renamed()),
            log_reader=LogReader(loop=loop),

            hid=Hid(**config.hid._unpack_renamed()),
            atx=Atx(**config.atx._unpack_renamed()),
            msd=MassStorageDevice(loop=loop, **config.msd._unpack_renamed()),
            streamer=Streamer(loop=loop, **config.streamer._unpack_renamed()),

            loop=loop,
        ).run(**config.server._unpack_renamed())
    get_logger().info("Bye-bye")
