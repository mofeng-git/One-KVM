import sys
import asyncio
import logging
import time

from typing import List
from typing import Dict
from typing import AsyncGenerator

import systemd.journal


# =====
def get_logger(depth: int=1) -> logging.Logger:
    frame = sys._getframe(1)  # pylint: disable=protected-access
    frames = []
    while frame:
        frames.append(frame)
        frame = frame.f_back
        if len(frames) - 1 >= depth:
            break
    name = frames[depth].f_globals["__name__"]
    return logging.getLogger(name)


class Log:
    def __init__(
        self,
        services: List[str],
        loop: asyncio.AbstractEventLoop,
    ) -> None:

        self.__services = services
        self.__loop = loop

    async def log(self, seek: int, follow: bool) -> AsyncGenerator[Dict, None]:
        reader = systemd.journal.Reader()
        reader.this_boot()
        reader.this_machine()
        reader.log_level(systemd.journal.LOG_DEBUG)
        for service in self.__services:
            reader.add_match(_SYSTEMD_UNIT=service)
        if seek > 0:
            reader.seek_realtime(float(time.time() - seek))

        for entry in reader:
            yield self.__entry_to_record(entry)

        while follow:
            entry = reader.get_next()
            if entry:
                yield self.__entry_to_record(entry)
            else:
                await asyncio.sleep(1)

    def __entry_to_record(self, entry: Dict) -> Dict[str, Dict]:
        return {
            "dt": entry["__REALTIME_TIMESTAMP"],
            "service": entry["_SYSTEMD_UNIT"],
            "msg": entry["MESSAGE"].rstrip(),
        }
