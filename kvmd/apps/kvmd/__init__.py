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
    config = init("kvmd")[2].kvmd
    with gpio.bcm():
        loop = asyncio.get_event_loop()

        auth_manager = AuthManager(
            htpasswd_path=config.auth.htpasswd,
        )

        info_manager = InfoManager(
            meta_path=config.info.meta,
            extras_path=config.info.extras,
            loop=loop,
        )

        log_reader = LogReader(loop)

        hid = Hid(
            reset=config.hid.pinout.reset,
            reset_delay=config.hid.reset_delay,

            device_path=config.hid.device,
            speed=config.hid.speed,
            read_timeout=config.hid.read_timeout,
            read_retries=config.hid.read_retries,
            common_retries=config.hid.common_retries,
            retries_delay=config.hid.retries_delay,
            noop=config.hid.noop,

            state_poll=config.hid.state_poll,
        )

        atx = Atx(
            power_led=config.atx.pinout.power_led,
            hdd_led=config.atx.pinout.hdd_led,
            power_switch=config.atx.pinout.power_switch,
            reset_switch=config.atx.pinout.reset_switch,

            click_delay=config.atx.click_delay,
            long_click_delay=config.atx.long_click_delay,
            state_poll=config.atx.state_poll,
        )

        msd = MassStorageDevice(
            target=config.msd.pinout.target,
            reset=config.msd.pinout.reset,

            device_path=config.msd.device,
            init_delay=config.msd.init_delay,
            reset_delay=config.msd.reset_delay,
            write_meta=config.msd.write_meta,

            loop=loop,
        )

        streamer = Streamer(
            cap_power=config.streamer.pinout.cap,
            conv_power=config.streamer.pinout.conv,
            sync_delay=config.streamer.sync_delay,
            init_delay=config.streamer.init_delay,
            init_restart_after=config.streamer.init_restart_after,
            state_poll=config.streamer.state_poll,

            quality=config.streamer.quality,
            desired_fps=config.streamer.desired_fps,

            host=config.streamer.host,
            port=config.streamer.port,
            unix_path=config.streamer.unix,
            timeout=config.streamer.timeout,

            cmd=config.streamer.cmd,

            loop=loop,
        )

        Server(
            auth_manager=auth_manager,
            info_manager=info_manager,
            log_reader=log_reader,

            hid=hid,
            atx=atx,
            msd=msd,
            streamer=streamer,

            access_log_format=config.server.access_log_format,
            heartbeat=config.server.heartbeat,
            streamer_shutdown_delay=config.streamer.shutdown_delay,
            msd_chunk_size=config.msd.chunk_size,

            loop=loop,
        ).run(
            host=config.server.host,
            port=config.server.port,
            unix_path=config.server.unix,
            unix_rm=config.server.unix_rm,
            unix_mode=config.server.unix_mode,
        )

    get_logger().info("Bye-bye")
