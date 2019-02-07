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
            htpasswd_path=str(config.get("auth", {}).get("htpasswd", "/etc/kvmd/htpasswd")),
        )

        info_manager = InfoManager(
            meta_path=str(config.get("info", {}).get("meta", "/etc/kvmd/meta.yaml")),
            extras_path=str(config.get("info", {}).get("extras", "/usr/share/kvmd/extras")),
            loop=loop,
        )

        log_reader = LogReader(loop)

        hid = Hid(
            reset=int(config["hid"]["pinout"]["reset"]),
            reset_delay=float(config["hid"].get("reset_delay", 0.1)),

            device_path=str(config["hid"]["device"]),
            speed=int(config["hid"].get("speed", 115200)),
            read_timeout=float(config["hid"].get("read_timeout", 2)),
            read_retries=int(config["hid"].get("read_retries", 10)),
            common_retries=int(config["hid"].get("common_retries", 100)),
            retries_delay=float(config["hid"].get("retries_delay", 0.1)),
            noop=bool(config["hid"].get("noop", False)),

            state_poll=float(config["hid"].get("state_poll", 0.1)),
        )

        atx = Atx(
            power_led=int(config["atx"]["pinout"]["power_led"]),
            hdd_led=int(config["atx"]["pinout"]["hdd_led"]),

            power_switch=int(config["atx"]["pinout"]["power_switch"]),
            reset_switch=int(config["atx"]["pinout"]["reset_switch"]),
            click_delay=float(config["atx"].get("click_delay", 0.1)),
            long_click_delay=float(config["atx"].get("long_click_delay", 5.5)),
            state_poll=float(config["atx"].get("state_poll", 0.1)),
        )

        msd = MassStorageDevice(
            target=int(config["msd"]["pinout"]["target"]),
            reset=int(config["msd"]["pinout"]["reset"]),

            device_path=str(config["msd"]["device"]),
            init_delay=float(config["msd"].get("init_delay", 2)),
            reset_delay=float(config["msd"].get("reset_delay", 1)),
            write_meta=bool(config["msd"].get("write_meta", True)),

            loop=loop,
        )

        streamer = Streamer(
            cap_power=int(config["streamer"].get("pinout", {}).get("cap", -1)),
            conv_power=int(config["streamer"].get("pinout", {}).get("conv", -1)),
            sync_delay=float(config["streamer"].get("sync_delay", 1)),
            init_delay=float(config["streamer"].get("init_delay", 1)),
            init_restart_after=float(config["streamer"].get("init_restart_after", 0)),
            state_poll=float(config["streamer"].get("state_poll", 1)),

            quality=int(config["streamer"].get("quality", 80)),
            desired_fps=int(config["streamer"].get("desired_fps", 0)),

            host=str(config["streamer"].get("host", "localhost")),
            port=int(config["streamer"].get("port", 0)),
            unix_path=str(config["streamer"].get("unix", "")),
            timeout=float(config["streamer"].get("timeout", 2)),

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

            access_log_format=str(config["server"].get(
                "access_log_format",
                "[%P / %{X-Real-IP}i] '%r' => %s; size=%b --- referer='%{Referer}i'; user_agent='%{User-Agent}i'",
            )),
            heartbeat=float(config["server"].get("heartbeat", 3)),
            streamer_shutdown_delay=float(config["streamer"].get("shutdown_delay", 10)),
            msd_chunk_size=int(config["msd"].get("chunk_size", 65536)),

            loop=loop,
        ).run(
            host=str(config["server"].get("host", "localhost")),
            port=int(config["server"].get("port", 0)),
            unix_path=str(config["server"].get("unix", "")),
            unix_rm=bool(config["server"].get("unix_rm", False)),
            unix_mode=int(config["server"].get("unix_mode", 0)),
        )

    get_logger().info("Bye-bye")
