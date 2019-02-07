import asyncio

from ...application import init
from ...logging import get_logger

from ... import gpio

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
    config = init()["kvmd"]
    with gpio.bcm():
        loop = asyncio.get_event_loop()

        auth_manager = AuthManager(
            htpasswd_path=str(config["auth"]["htpasswd"]),
        )

        info_manager = InfoManager(
            meta_path=str(config["info"]["meta"]),
            extras_path=str(config["info"]["extras"]),
            loop=loop,
        )

        log_reader = LogReader(loop)

        hid = Hid(
            reset=int(config["hid"]["pinout"]["reset"]),

            reset_delay=float(config["hid"]["reset_delay"]),

            device_path=str(config["hid"]["device"]),
            speed=int(config["hid"]["speed"]),
            read_timeout=float(config["hid"]["read_timeout"]),
            read_retries=int(config["hid"]["read_retries"]),
            common_retries=int(config["hid"]["common_retries"]),
            retries_delay=float(config["hid"]["retries_delay"]),
            noop=bool(config["hid"].get("noop", False)),

            state_poll=float(config["hid"]["state_poll"]),
        )

        atx = Atx(
            power_led=int(config["atx"]["pinout"]["power_led"]),
            hdd_led=int(config["atx"]["pinout"]["hdd_led"]),

            power_switch=int(config["atx"]["pinout"]["power_switch"]),
            reset_switch=int(config["atx"]["pinout"]["reset_switch"]),
            click_delay=float(config["atx"]["click_delay"]),
            long_click_delay=float(config["atx"]["long_click_delay"]),
            state_poll=float(config["atx"]["state_poll"]),
        )

        msd = MassStorageDevice(
            target=int(config["msd"]["pinout"]["target"]),
            reset=int(config["msd"]["pinout"]["reset"]),

            device_path=str(config["msd"]["device"]),
            init_delay=float(config["msd"]["init_delay"]),
            reset_delay=float(config["msd"]["reset_delay"]),
            write_meta=bool(config["msd"]["write_meta"]),

            loop=loop,
        )

        streamer = Streamer(
            cap_power=int(config["streamer"]["pinout"]["cap"]),
            conv_power=int(config["streamer"]["pinout"]["conv"]),
            sync_delay=float(config["streamer"]["sync_delay"]),
            init_delay=float(config["streamer"]["init_delay"]),
            init_restart_after=float(config["streamer"]["init_restart_after"]),
            state_poll=float(config["streamer"]["state_poll"]),

            quality=int(config["streamer"]["quality"]),
            desired_fps=int(config["streamer"]["desired_fps"]),

            host=str(config["streamer"].get("host", "localhost")),
            port=int(config["streamer"].get("port", 0)),
            unix_path=str(config["streamer"].get("unix", "")),
            timeout=float(config["streamer"]["timeout"]),

            cmd=list(map(str, config["streamer"]["cmd"])),

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

            access_log_format=str(config["server"]["access_log_format"]),
            heartbeat=float(config["server"]["heartbeat"]),
            streamer_shutdown_delay=float(config["streamer"]["shutdown_delay"]),
            msd_chunk_size=int(config["msd"]["chunk_size"]),

            loop=loop,
        ).run(
            host=str(config["server"].get("host", "localhost")),
            port=int(config["server"].get("port", 0)),
            unix_path=str(config["server"].get("unix", "")),
            unix_rm=bool(config["server"].get("unix_rm", False)),
            unix_mode=int(config["server"].get("unix_mode", 0)),
        )

    get_logger().info("Bye-bye")
