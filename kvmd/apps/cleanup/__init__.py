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
import signal
import time

from typing import List
from typing import Optional

import psutil

from ...logging import get_logger

from ...yamlconf import Section

from ... import gpio

from .. import init


# =====
def _clear_gpio(config: Section) -> None:
    logger = get_logger(0)

    with gpio.bcm():
        for (name, pin) in [
            *([
                ("atx_gpio/power_switch", config.atx.power_switch_pin),
                ("atx_gpio/reset_switch", config.atx.reset_switch_pin),
            ] if config.atx.type == "gpio" else []),

            *([
                ("msd_relay/target", config.msd.target_pin),
                ("msd_relay/reset", config.msd.reset_pin),
            ] if config.msd.type == "relay" else []),
        ]:
            if pin >= 0:
                logger.info("Writing 0 to GPIO pin=%d (%s)", pin, name)
                try:
                    gpio.set_output(pin, False)
                except Exception:
                    logger.exception("Can't clear GPIO pin=%d (%s)", pin, name)


def _kill_streamer(config: Section) -> None:
    logger = get_logger(0)

    if config.streamer.process_name_prefix:
        prefix = config.streamer.process_name_prefix + ":"
        logger.info("Trying to find and kill the streamer %r ...", prefix + " <app>")

        for proc in psutil.process_iter():
            attrs = proc.as_dict(attrs=["name"])
            if attrs.get("name", "").startswith(prefix):
                try:
                    proc.send_signal(signal.SIGTERM)
                except Exception:
                    logger.exception("Can't send SIGTERM to streamer with pid=%d", proc.pid)
                time.sleep(3)
                if proc.is_running():
                    try:
                        proc.send_signal(signal.SIGKILL)
                    except Exception:
                        logger.exception("Can't send SIGKILL to streamer with pid=%d", proc.pid)


def _remove_sockets(config: Section) -> None:
    logger = get_logger(0)
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


# =====
def main(argv: Optional[List[str]]=None) -> None:
    config = init(
        prog="kvmd-cleanup",
        description="Kill KVMD and clear resources",
        argv=argv,
        load_hid=True,
        load_atx=True,
        load_msd=True,
        load_gpio=True,
    )[2].kvmd

    logger = get_logger(0)
    logger.info("Cleaning up ...")

    for method in [
        _clear_gpio,
        _kill_streamer,
        _remove_sockets,
    ]:
        try:
            method(config)
        except Exception:
            pass

    logger.info("Bye-bye")
