import os
import subprocess
import time

from ...application import init
from ...logging import get_logger

from ... import gpio


# =====
def main() -> None:
    config = init()
    logger = get_logger(0)

    logger.info("Cleaning up ...")
    with gpio.bcm():
        for (key, pin) in [
            *config["atx"]["switches"]["pinout"].items(),
            *config["video"]["pinout"].items(),
        ]:
            if pin > 0:
                logger.info("Writing value=0 to pin=%d (%s)", pin, key)
                gpio.set_output(pin, initial=False)

    streamer = os.path.basename(config["video"]["cmd"][0])
    logger.info("Trying to find and kill %r ...", streamer)
    try:
        subprocess.check_output(["killall", streamer], stderr=subprocess.STDOUT)
        time.sleep(3)
        subprocess.check_output(["killall", "-9", streamer], stderr=subprocess.STDOUT)
    except subprocess.CalledProcessError:
        pass

    logger.info("Bye-bye")
