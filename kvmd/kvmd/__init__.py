import asyncio
import logging

from .application import init

from .atx import Atx
from .streamer import Streamer
from .ps2 import Ps2Keyboard
from .server import Server

from . import gpio


# =====
def main() -> None:
    config = init()
    with gpio.bcm():
        loop = asyncio.get_event_loop()

        atx = Atx(
            power_led=config["atx"]["leds"]["pinout"]["power"],
            hdd_led=config["atx"]["leds"]["pinout"]["hdd"],
            power_switch=config["atx"]["switches"]["pinout"]["power"],
            reset_switch=config["atx"]["switches"]["pinout"]["reset"],
            click_delay=config["atx"]["switches"]["click_delay"],
            long_click_delay=config["atx"]["switches"]["long_click_delay"],
        )

        streamer = Streamer(
            cap_power=config["video"]["pinout"]["cap"],
            vga_power=config["video"]["pinout"]["vga"],
            sync_delay=config["video"]["sync_delay"],
            mjpg_streamer=config["video"]["mjpg_streamer"],
            loop=loop,
        )

        keyboard = Ps2Keyboard(
            clock=config["keyboard"]["pinout"]["clock"],
            data=config["keyboard"]["pinout"]["data"],
            pulse=config["keyboard"]["pulse"],
        )

        Server(
            atx=atx,
            streamer=streamer,
            keyboard=keyboard,
            heartbeat=config["server"]["heartbeat"],
            atx_leds_poll=config["atx"]["leds"]["poll"],
            video_shutdown_delay=config["video"]["shutdown_delay"],
            loop=loop,
        ).run(
            host=config["server"]["host"],
            port=config["server"]["port"],
        )
    logging.getLogger(__name__).info("Bye-bye")
