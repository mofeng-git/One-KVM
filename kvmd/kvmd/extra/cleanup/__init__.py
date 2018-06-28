import logging

from RPi import GPIO

from ...application import init


# =====
_logger = logging.getLogger(__name__)


def main() -> None:
    config = init()
    _logger.info("Cleaning up ...")
    GPIO.setmode(GPIO.BCM)
    for (key, pin) in [
        *config["atx"]["switches"]["pinout"].items(),
        *config["video"]["pinout"].items(),
    ]:
        _logger.info("Writing value=0 to pin=%d (%s)", pin, key)
        GPIO.output(pin, False)
    GPIO.cleanup()
    _logger.info("Done!")
