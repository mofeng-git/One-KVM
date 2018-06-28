import subprocess
import logging
import time

from ...application import init

from ... import gpio


# =====
_logger = logging.getLogger(__name__)


def main() -> None:
    config = init()
    _logger.info("Cleaning up ...")
    with gpio.bcm():
        for (key, pin) in [
            *config["atx"]["switches"]["pinout"].items(),
            *config["video"]["pinout"].items(),
        ]:
            _logger.info("Writing value=0 to pin=%d (%s)", pin, key)
            gpio.write(pin, False)

    _logger.info("Trying to find and kill mjpg_streamer ...")
    try:
        subprocess.check_output(["killall", "mjpg_streamer"], stderr=subprocess.STDOUT)
        time.sleep(3)
        subprocess.check_output(["killall", "-9", "mjpg_streamer"], stderr=subprocess.STDOUT)
    except subprocess.CalledProcessError:
        pass

    _logger.info("Bye-bye")
