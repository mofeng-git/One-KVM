import os
import subprocess
import time

from ...logging import get_logger

from ... import gpio

from .. import init


# =====
def main() -> None:
    config = init("kvmd-cleanup")[2].kvmd
    logger = get_logger(0)

    logger.info("Cleaning up ...")
    with gpio.bcm():
        for (name, pin) in [
            ("hid_reset", config.hid.pinout.reset),
            ("msd_target", config.msd.pinout.target),
            ("msd_reset", config.msd.pinout.reset),
            ("atx_power_switch", config.atx.pinout.power_switch),
            ("atx_reset_switch", config.atx.pinout.reset_switch),
            ("streamer_cap", config.streamer.pinout.cap),
            ("streamer_conv", config.streamer.pinout.conv),
        ]:
            if pin > 0:
                logger.info("Writing value=0 to pin=%d (%s)", pin, name)
                gpio.set_output(pin, initial=False)

    streamer = os.path.basename(config.streamer.cmd[0])
    logger.info("Trying to find and kill %r ...", streamer)
    try:
        subprocess.check_output(["killall", streamer], stderr=subprocess.STDOUT)
        time.sleep(3)
        subprocess.check_output(["killall", "-9", streamer], stderr=subprocess.STDOUT)
    except subprocess.CalledProcessError:
        pass

    unix_path = config.server.unix
    if unix_path and os.path.exists(unix_path):
        logger.info("Removing socket %r ...", unix_path)
        os.remove(unix_path)

    logger.info("Bye-bye")
