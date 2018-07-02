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
            power_led=int(config["atx"]["leds"]["pinout"]["power"]),
            hdd_led=int(config["atx"]["leds"]["pinout"]["hdd"]),
            power_switch=int(config["atx"]["switches"]["pinout"]["power"]),
            reset_switch=int(config["atx"]["switches"]["pinout"]["reset"]),
            click_delay=float(config["atx"]["switches"]["click_delay"]),
            long_click_delay=float(config["atx"]["switches"]["long_click_delay"]),
        )

        msd = MassStorageDevice(
            bind=str(config["msd"]["bind"]),
            init_delay=float(config["msd"]["init_delay"]),
            loop=loop,
        )

        streamer = Streamer(
            cap_power=int(config["video"]["pinout"]["cap"]),
            conv_power=int(config["video"]["pinout"]["conv"]),
            sync_delay=float(config["video"]["sync_delay"]),
            cmd=list(map(str, config["video"]["cmd"])),
            loop=loop,
        )

        Server(
            keyboard=keyboard,
            atx=atx,
            msd=msd,
            streamer=streamer,
            heartbeat=float(config["server"]["heartbeat"]),
            atx_leds_poll=float(config["atx"]["leds"]["poll"]),
            video_shutdown_delay=float(config["video"]["shutdown_delay"]),
            msd_chunk_size=int(config["msd"]["chunk_size"]),
            loop=loop,
        ).run(
            host=str(config["server"]["host"]),
            port=int(config["server"]["port"]),
        )

    get_logger().info("Bye-bye")
