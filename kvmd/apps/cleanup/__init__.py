# ========================================================================== #
#                                                                            #
#    KVMD - The The main Pi-KVM daemon.                                      #
#                                                                            #
#    Copyright (C) 2018  Maxim Devaev <mdevaev@gmail.com>                    #
#                                                                            #
#    This program is free software: you can redistribute it and/or modify    #
#    it under the terms of the GNU General Public License as published by    #
#    the Free Software Foundation, either version 3 of the License, or       #
#    (at your option) any later version.                                     #
#                                                                            #
#    This program is distributed in the hope that it will be useful,         #
#    but WITHOUT ANY WARRANTY; without even the implied warranty of          #
#    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the           #
#    GNU General Public License for more details.                            #
#                                                                            #
#    You should have received a copy of the GNU General Public License       #
#    along with this program.  If not, see <https://www.gnu.org/licenses/>.  #
#                                                                            #
# ========================================================================== #


import os
import subprocess
import time

from ...logging import get_logger

from ... import gpio

from .. import init


# =====
def main() -> None:
    config = init("kvmd-cleanup", description="Kill KVMD and clear resources")[2].kvmd
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
