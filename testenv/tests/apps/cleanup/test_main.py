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


import os
import multiprocessing
import time

from typing import Literal

import setproctitle

from kvmd.apps.cleanup import main


# =====
def test_ok(tmpdir) -> None:  # type: ignore
    _ = Literal  # Makes liters happy
    queue: "multiprocessing.Queue[Literal[True]]" = multiprocessing.Queue()

    ustreamer_sock_path = os.path.abspath(str(tmpdir.join("ustreamer-fake.sock")))
    open(ustreamer_sock_path, "w").close()  # pylint: disable=consider-using-with
    kvmd_sock_path = os.path.abspath(str(tmpdir.join("kvmd-fake.sock")))
    open(kvmd_sock_path, "w").close()  # pylint: disable=consider-using-with

    def ustreamer_fake() -> None:
        setproctitle.setproctitle("kvmd/streamer: /usr/bin/ustreamer")
        queue.put(True)
        while True:
            time.sleep(1)

    proc = multiprocessing.Process(target=ustreamer_fake, daemon=True)
    proc.start()
    assert queue.get(timeout=5)

    assert proc.is_alive()
    main([
        "kvmd-cleanup",
        "--set-options",
        f"kvmd/server/unix={kvmd_sock_path}",
        f"kvmd/streamer/unix={ustreamer_sock_path}",
        "--run",
    ])

    assert not os.path.exists(ustreamer_sock_path)
    assert not os.path.exists(kvmd_sock_path)

    assert not proc.is_alive()
    proc.join()
