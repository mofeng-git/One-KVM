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
                gpio.write(pin, False)

    logger.info("Trying to find and kill mjpg_streamer ...")
    try:
        subprocess.check_output(["killall", "mjpg_streamer"], stderr=subprocess.STDOUT)
        time.sleep(3)
        subprocess.check_output(["killall", "-9", "mjpg_streamer"], stderr=subprocess.STDOUT)
    except subprocess.CalledProcessError:
        pass

    logger.info("Bye-bye")
