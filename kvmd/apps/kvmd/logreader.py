import re
import asyncio
import time

from typing import Dict
from typing import AsyncGenerator

import systemd.journal


# =====
class LogReader:
    def __init__(self, loop: asyncio.AbstractEventLoop) -> None:
        self.__loop = loop

    async def poll_log(self, seek: int, follow: bool) -> AsyncGenerator[Dict, None]:
        reader = systemd.journal.Reader()
        reader.this_boot()
        reader.this_machine()
        reader.log_level(systemd.journal.LOG_DEBUG)

        services = set(
            service
            for service in systemd.journal.Reader().query_unique("_SYSTEMD_UNIT")
            if re.match(r"kvmd(-\w+)?\.service", service)
        ).union(["kvmd.service"])

        for service in services:
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
