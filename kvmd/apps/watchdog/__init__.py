# ========================================================================== #
#                                                                            #
#    KVMD - The main PiKVM daemon.                                           #
#                                                                            #
#    Copyright (C) 2018-2023  Maxim Devaev <mdevaev@gmail.com>               #
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


import argparse
import errno
import time

from ...logging import get_logger

from ...yamlconf import Section

from ... import env

from .. import init


# =====
class RtcIsNotAvailableError(Exception):
    pass


# =====
def _join_rtc(rtc: int, key: str) -> str:
    return f"{env.SYSFS_PREFIX}/sys/class/rtc/rtc{rtc}/{key}"


def _read_int(rtc: int, key: str) -> int:
    with open(_join_rtc(rtc, key)) as file:
        return int(file.read().strip() or "0")


def _write_int(rtc: int, key: str, value: int) -> None:
    with open(_join_rtc(rtc, key), "w") as file:
        file.write(str(value))


def _reset_alarm(rtc: int, timeout: int) -> None:
    try:
        now = _read_int(rtc, "since_epoch")
    except OSError as err:
        if err.errno != errno.EINVAL:
            raise
        raise RtcIsNotAvailableError("Can't read since_epoch right now")
    if now == 0:
        raise RtcIsNotAvailableError("Current UNIX time == 0")
    try:
        for wake in [0, now + timeout]:
            _write_int(rtc, "wakealarm", wake)
    except OSError as err:
        if err.errno != errno.EIO:
            raise
        raise RtcIsNotAvailableError("IO error, probably the supercapacitor is not charged")


# =====
def _cmd_run(config: Section) -> None:
    logger = get_logger(0)
    logger.info("Running watchdog loop on RTC%d ...", config.rtc)
    fail = False
    try:
        while True:
            try:
                _reset_alarm(config.rtc, config.timeout)
            except RtcIsNotAvailableError as err:
                if not fail:
                    logger.error("RTC%d is not available now: %s; waiting ...", config.rtc, err)
                    fail = True
            else:
                if fail:
                    logger.info("RTC%d is available, working ...", config.rtc)
                    fail = False
            time.sleep(config.interval)
    except (SystemExit, KeyboardInterrupt):
        if not fail:
            _reset_alarm(config.rtc, config.timeout)
            logger.info("The watchdog remains alarmed. Use 'kvmd-watchdog cancel' to disarm it")
    logger.info("Bye-bye")


def _cmd_cancel(config: Section) -> None:
    _write_int(config.rtc, "wakealarm", 0)


# =====
def main(argv: (list[str] | None)=None) -> None:
    (parent_parser, argv, config) = init(add_help=False, argv=argv)
    parser = argparse.ArgumentParser(
        prog="kvmd-watchdog",
        description="RTC-based hardware watchdog",
        parents=[parent_parser],
    )
    parser.set_defaults(cmd=(lambda *_: parser.print_help()))
    subparsers = parser.add_subparsers()

    cmd_run_parser = subparsers.add_parser("run", help="Run watchdog loop")
    cmd_run_parser.set_defaults(cmd=_cmd_run)

    cmd_cancel_parser = subparsers.add_parser("cancel", help="Cancel armed timeout")
    cmd_cancel_parser.set_defaults(cmd=_cmd_cancel)

    options = parser.parse_args(argv[1:])
    options.cmd(config.watchdog)
