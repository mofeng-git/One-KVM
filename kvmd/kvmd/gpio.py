import contextlib
import logging

from typing import Generator

from RPi import GPIO


# =====
_logger = logging.getLogger(__name__)


@contextlib.contextmanager
def bcm() -> Generator[None, None, None]:
    GPIO.setmode(GPIO.BCM)
    _logger.info("Configured GPIO mode as BCM")
    try:
        yield
    finally:
        GPIO.cleanup()
        _logger.info("GPIO cleaned")


def set_output_zeroed(pin: int) -> int:
    GPIO.setup(pin, GPIO.OUT)
    GPIO.output(pin, False)
    return pin


def set_input(pin: int) -> int:
    GPIO.setup(pin, GPIO.IN)
    return pin


def read(pin: int) -> bool:
    return bool(GPIO.input(pin))


def write(pin: int, flag: bool) -> None:
    GPIO.output(pin, flag)
