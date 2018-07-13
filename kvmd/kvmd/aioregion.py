import types

from typing import Type


# =====
class RegionIsBusyError(Exception):
    pass


class AioExclusiveRegion:
    def __init__(self) -> None:
        self.__busy = False

    def enter(self) -> None:
        if not self.__busy:
            self.__busy = True
            return
        raise RegionIsBusyError()

    def exit(self) -> None:
        self.__busy = False

    def __enter__(self) -> None:
        self.enter()

    def __exit__(
        self,
        _exc_type: Type[BaseException],
        _exc: BaseException,
        _tb: types.TracebackType,
    ) -> None:
        self.exit()
