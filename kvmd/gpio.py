import contextlib

from typing import Generator

from RPi import GPIO

from .logging import get_logger


# =====
@contextlib.contextmanager
def bcm() -> Generator[None, None, None]:
    logger = get_logger(2)
    GPIO.setmode(GPIO.BCM)
    logger.info("Configured GPIO mode as BCM")
    try:
        yield
    finally:
        GPIO.cleanup()
        logger.info("GPIO cleaned")


def set_output(pin: int, initial: bool=False) -> int:
    GPIO.setup(pin, GPIO.OUT, initial=initial)
    return pin


def set_input(pin: int) -> int:
    GPIO.setup(pin, GPIO.IN)
    return pin


def read(pin: int) -> bool:
    return bool(GPIO.input(pin))


def write(pin: int, flag: bool) -> None:
    GPIO.output(pin, flag)
