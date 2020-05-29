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


import asyncio
import asyncio.subprocess
import signal

from typing import Tuple
from typing import List


# =====
async def run_process(cmd: List[str], err_to_null: bool=False) -> asyncio.subprocess.Process:  # pylint: disable=no-member
    return (await asyncio.create_subprocess_exec(
        *cmd,
        stdout=asyncio.subprocess.PIPE,
        stderr=(asyncio.subprocess.DEVNULL if err_to_null else asyncio.subprocess.STDOUT),
        preexec_fn=preexec_ignore_sigint,
    ))


async def read_process(cmd: List[str], err_to_null: bool=False) -> Tuple[asyncio.subprocess.Process, str]:  # pylint: disable=no-member
    proc = await run_process(cmd, err_to_null)
    (stdout, _) = await proc.communicate()
    return (proc, stdout.decode(errors="ignore").strip())


def preexec_ignore_sigint() -> None:
    signal.signal(signal.SIGINT, signal.SIG_IGN)
