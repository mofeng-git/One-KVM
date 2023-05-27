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


import subprocess

from .logging import get_logger

from . import tools
from . import aioproc


# =====
async def remount(name: str, base_cmd: list[str], rw: bool) -> bool:
    logger = get_logger(1)
    mode = ("rw" if rw else "ro")
    cmd = [
        part.format(mode=mode)
        for part in base_cmd
    ]
    logger.info("Remounting %s storage to %s: %s ...", name, mode.upper(), tools.cmdfmt(cmd))
    try:
        proc = await aioproc.log_process(cmd, logger)
        if proc.returncode != 0:
            assert proc.returncode is not None
            raise subprocess.CalledProcessError(proc.returncode, cmd)
    except Exception as err:
        logger.error("Can't remount %s storage: %s", name, tools.efmt(err))
        return False
    return True
