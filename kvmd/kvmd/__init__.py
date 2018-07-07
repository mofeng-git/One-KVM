import asyncio

from .application import init
from .logging import get_logger

from .ps2 import Ps2Keyboard
from .atx import Atx
from .msd import MassStorageDevice
from .streamer import Streamer
from .server import Server

from . import gpio


# =====
def main() -> None:
    config = init()
    with gpio.bcm():
        loop = asyncio.get_event_loop()

        keyboard = Ps2Keyboard(
            clock=int(config["keyboard"]["pinout"]["clock"]),
            data=int(config["keyboard"]["pinout"]["data"]),
            pulse=float(config["keyboard"]["pulse"]),
        )

        atx = Atx(
            power_led=int(config["atx"]["pinout"]["power_led"]),
            hdd_led=int(config["atx"]["pinout"]["hdd_led"]),
            power_switch=int(config["atx"]["pinout"]["power_switch"]),
            reset_switch=int(config["atx"]["pinout"]["reset_switch"]),
            click_delay=float(config["atx"]["click_delay"]),
            long_click_delay=float(config["atx"]["long_click_delay"]),
        )

        msd = MassStorageDevice(
            device_path=str(config["msd"]["device"]),
            init_delay=float(config["msd"]["init_delay"]),
            write_meta=bool(config["msd"]["write_meta"]),
            loop=loop,
        )

        streamer = Streamer(
            cap_power=int(config["streamer"]["pinout"]["cap"]),
            conv_power=int(config["streamer"]["pinout"]["conv"]),
            sync_delay=float(config["streamer"]["sync_delay"]),
            init_delay=float(config["streamer"]["init_delay"]),
            width=int(config["streamer"]["size"]["width"]),
            height=int(config["streamer"]["size"]["height"]),
            cmd=list(map(str, config["streamer"]["cmd"])),
            loop=loop,
        )

        Server(
            keyboard=keyboard,
            atx=atx,
            msd=msd,
            streamer=streamer,
            heartbeat=float(config["server"]["heartbeat"]),
            atx_state_poll=float(config["atx"]["state_poll"]),
            streamer_shutdown_delay=float(config["streamer"]["shutdown_delay"]),
            msd_chunk_size=int(config["msd"]["chunk_size"]),
            loop=loop,
        ).run(
            host=str(config["server"]["host"]),
            port=int(config["server"]["port"]),
        )

    get_logger().info("Bye-bye")
