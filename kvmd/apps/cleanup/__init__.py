# ========================================================================== #
#                                                                            #
#    KVMD - The main Pi-KVM daemon.                                          #
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

from typing import List
from typing import Optional

from ...logging import get_logger

from ... import gpio

from .. import init


# =====
def main(argv: Optional[List[str]]=None) -> None:
    config = init(
        prog="kvmd-cleanup",
        description="Kill KVMD and clear resources",
        argv=argv,
        sections=["logging", "kvmd"],
        with_hid=True,
        with_atx=True,
        with_msd=True,
    )[2].kvmd

    logger = get_logger(0)

    logger.info("Cleaning up ...")
    with gpio.bcm():
        for (name, pin) in [
            *([
                ("tty_hid_reset_pin", config.hid.reset_pin),
            ] if config.hid.type == "tty" else []),

            *([
                ("gpio_atx_power_switch_pin", config.atx.power_switch_pin),
                ("gpio_atx_reset_switch_pin", config.atx.reset_switch_pin),
            ] if config.atx.type == "gpio" else []),

            *([
                ("relay_msd_target_pin", config.msd.target_pin),
                ("relay_msd_reset_pin", config.msd.reset_pin),
            ] if config.msd.type == "relay" else []),

            ("streamer_cap_pin", config.streamer.cap_pin),
            ("streamer_conv_pin", config.streamer.conv_pin),
        ]:
            if pin >= 0:
                logger.info("Writing value=0 to GPIO pin=%d (%s)", pin, name)
                try:
                    gpio.set_output(pin, initial=False)
                except Exception:
                    logger.exception("Can't clear GPIO pin=%d (%s)", pin, name)

    streamer = os.path.basename(config.streamer.cmd[0])
    logger.info("Trying to find and kill %r ...", streamer)
    try:
        subprocess.check_output(["killall", streamer], stderr=subprocess.STDOUT)
        time.sleep(3)
        subprocess.check_output(["killall", "-9", streamer], stderr=subprocess.STDOUT)
    except subprocess.CalledProcessError:  # pragma: nocover
        pass

    for (owner, unix_path) in [
        ("KVMD", config.server.unix),
        ("streamer", config.streamer.unix),
    ]:
        if unix_path and os.path.exists(unix_path):
            logger.info("Removing %s socket %r ...", owner, unix_path)
            try:
                os.remove(unix_path)
            except Exception:  # pragma: nocover
                logger.exception("Can't remove %s socket %r", owner, unix_path)

    logger.info("Bye-bye")
