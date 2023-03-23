# ========================================================================== #
#                                                                            #
#    KVMD - The main PiKVM daemon.                                           #
#                                                                            #
#    Copyright (C) 2020  Maxim Devaev <mdevaev@gmail.com>                    #
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


import pkgutil
import functools
import importlib.machinery

import Xlib.keysymdef

from ..logging import get_logger

from .mappings import At1Key
from .mappings import WebModifiers
from .mappings import X11_TO_AT1
from .mappings import AT1_TO_WEB


# =====
class SymmapModifiers:
    SHIFT: int = 0x1
    ALTGR: int = 0x2
    CTRL: int = 0x4


def build_symmap(path: str) -> dict[int, dict[int, str]]:  # x11 keysym -> [(modifiers, webkey), ...]
    # https://github.com/qemu/qemu/blob/95a9457fd44ad97c518858a4e1586a5498f9773c/ui/keymaps.c
    logger = get_logger()

    symmap: dict[int, dict[int, str]] = {}
    for (src, items) in [
        (path, list(_read_keyboard_layout(path).items())),
        ("<builtin>", list(X11_TO_AT1.items())),
    ]:
        # Пока лучшая логика - самые первые записи в файле раскладки
        # должны иметь приоритет над следующими, а дефолтный маппинг
        # только дополняет отсутствующие значения.

        for (code, keys) in items:
            for key in keys:
                web_name = AT1_TO_WEB.get(key.code)
                if web_name is not None:
                    if (
                        (web_name in WebModifiers.SHIFTS and key.shift)  # pylint: disable=too-many-boolean-expressions
                        or (web_name in WebModifiers.ALTS and key.altgr)
                        or (web_name in WebModifiers.CTRLS and key.ctrl)
                    ):
                        logger.error("Invalid modifier key at mapping %s: %s / %s", src, web_name, key)
                        continue

                    modifiers = (
                        0
                        | (SymmapModifiers.SHIFT if key.shift else 0)
                        | (SymmapModifiers.ALTGR if key.altgr else 0)
                        | (SymmapModifiers.CTRL if key.ctrl else 0)
                    )
                    if code not in symmap:
                        symmap[code] = {}
                    symmap[code].setdefault(modifiers, web_name)
    return symmap


# =====
@functools.lru_cache()
def _get_keysyms() -> dict[str, int]:
    keysyms: dict[str, int] = {
        "EuroSign": 0x20AC,  # FIXME: https://github.com/python-xlib/python-xlib/pull/264
    }
    for (finder, module_name, _) in pkgutil.walk_packages(Xlib.keysymdef.__path__):
        if not isinstance(finder, importlib.machinery.FileFinder):
            continue
        loader = finder.find_module(module_name)
        if loader is None:
            continue
        module = loader.load_module(module_name)
        for keysym_name in dir(module):
            if keysym_name.startswith("XK_"):
                short_name = keysym_name[3:]
                if short_name.startswith("XF86_"):
                    short_name = "XF86" + short_name[5:]
                # assert short_name not in keysyms, short_name
                keysyms[short_name] = int(getattr(module, keysym_name))
    return keysyms


def _resolve_keysym(name: str) -> int:
    code = _get_keysyms().get(name)
    if code is not None:
        return code
    if len(name) == 5 and name[0] == "U":  # Try unicode Uxxxx
        try:
            return int(name[1:], 16)
        except ValueError:
            pass
    return 0


def _read_keyboard_layout(path: str) -> dict[int, list[At1Key]]:  # Keysym to evdev (at1)
    logger = get_logger(0)
    logger.info("Reading keyboard layout %s ...", path)

    with open(path) as file:
        lines = list(map(str.strip, file.read().split("\n")))

    layout: dict[int, list[At1Key]] = {}
    for (lineno, line) in enumerate(lines):
        if len(line) == 0 or line.startswith(("#", "map ", "include ")):
            continue

        parts = line.split()
        if len(parts) >= 2:
            x11_code = _resolve_keysym(parts[0])
            if x11_code == 0:
                continue

            try:
                at1_code = int(parts[1], 16)
            except ValueError as err:
                logger.error("Syntax error at %s:%d: %s", path, lineno, err)
                continue
            rest = parts[2:]

            if x11_code not in layout:
                layout[x11_code] = []
            layout[x11_code].append(At1Key(
                code=at1_code,
                shift=("shift" in rest),
                altgr=("altgr" in rest),
                ctrl=("ctrl" in rest),
            ))

            if "addupper" in rest:
                x11_code = _resolve_keysym(parts[0].upper())
                if x11_code != 0:
                    if x11_code not in layout:
                        layout[x11_code] = []
                    layout[x11_code].append(At1Key(
                        code=at1_code,
                        shift=True,
                    ))
    return layout
