# ========================================================================== #
#                                                                            #
#    KVMD - The main PiKVM daemon.                                           #
#                                                                            #
#    Copyright (C) 2018-2021  Maxim Devaev <mdevaev@gmail.com>               #
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


from typing import List

from ....logging import get_logger

from .... import aioproc

from .. import MsdError


# =====
async def remount_storage(base_cmd: List[str], rw: bool) -> None:
    logger = get_logger(0)
    mode = ("rw" if rw else "ro")
    cmd = [
        part.format(mode=mode)
        for part in base_cmd
    ]
    logger.info("Remounting internal storage to %s ...", mode.upper())
    try:
        await _run_helper(cmd)
    except Exception:
        logger.error("Can't remount internal storage")
        raise


async def unlock_drive(base_cmd: List[str]) -> None:
    logger = get_logger(0)
    logger.info("Unlocking the drive ...")
    try:
        await _run_helper(base_cmd)
    except Exception:
        logger.error("Can't unlock the drive")
        raise


# =====
async def _run_helper(cmd: List[str]) -> None:
    logger = get_logger(0)
    logger.info("Executing helper %s ...", cmd)
    proc = await aioproc.log_process(cmd, logger)
    if proc.returncode != 0:
        raise MsdError(f"Error while helper execution: pid={proc.pid}; retcode={proc.returncode}")
