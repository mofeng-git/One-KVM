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
import asyncio
import asyncio.subprocess
import logging

from typing import Tuple
from typing import List

import setproctitle

from .logging import get_logger


# =====
async def run_process(cmd: List[str], err_to_null: bool=False) -> asyncio.subprocess.Process:  # pylint: disable=no-member
    return (await asyncio.create_subprocess_exec(
        *cmd,
        stdout=asyncio.subprocess.PIPE,
        stderr=(asyncio.subprocess.DEVNULL if err_to_null else asyncio.subprocess.STDOUT),
        preexec_fn=os.setpgrp,
    ))


async def read_process(cmd: List[str], err_to_null: bool=False) -> Tuple[asyncio.subprocess.Process, str]:  # pylint: disable=no-member
    proc = await run_process(cmd, err_to_null)
    (stdout, _) = await proc.communicate()
    return (proc, stdout.decode(errors="ignore").strip())


async def log_process(cmd: List[str], logger: logging.Logger) -> asyncio.subprocess.Process:  # pylint: disable=no-member
    (proc, stdout) = await read_process(cmd)
    if stdout:
        log = (logger.info if proc.returncode == 0 else logger.error)
        for line in stdout.split("\n"):
            log("Console: %s", line)
    return proc


async def log_stdout_infinite(proc: asyncio.subprocess.Process, logger: logging.Logger) -> None:  # pylint: disable=no-member
    empty = 0
    async for line_bytes in proc.stdout:  # type: ignore
        line = line_bytes.decode(errors="ignore").strip()
        if line:
            logger.info("Console: %s", line)
            empty = 0
        else:
            empty += 1
            if empty == 100:  # asyncio bug
                raise RuntimeError("asyncio process: too many empty lines")


def rename_process(suffix: str, prefix: str="kvmd") -> None:
    setproctitle.setproctitle(f"{prefix}/{suffix}: {setproctitle.getproctitle()}")


def settle(name: str, suffix: str, prefix: str="kvmd") -> logging.Logger:
    logger = get_logger(1)
    logger.info("Started %s pid=%d", name, os.getpid())
    os.setpgrp()
    rename_process(suffix, prefix)
    return logger
