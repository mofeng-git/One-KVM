#!/usr/bin/env python3
# ========================================================================== #
#                                                                            #
#    KVMD - The main PiKVM daemon.                                           #
#                                                                            #
#    Copyright (C) 2018-2024  Maxim Devaev <mdevaev@gmail.com>               #
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


import sys
import csv
import textwrap
import dataclasses

import Xlib.keysymdef.latin1
import Xlib.keysymdef.miscellany
import Xlib.keysymdef.xf86
import Xlib.keysymdef.xkb

import mako.template


# =====
@dataclasses.dataclass(frozen=True)
class _UsbKey:
    code: int
    is_modifier: bool

    @property
    def arduino_modifier_code(self) -> int:
        # https://github.com/NicoHood/HID/blob/4bf6cd6/src/HID-APIs/DefaultKeyboardAPI.hpp#L31
        assert self.is_modifier
        code = self.code
        offset = 0
        while not (code & 0x1):
            code >>= 1
            offset += 1
            assert offset < 8
        return ((0xE << 4) | offset)


@dataclasses.dataclass(frozen=True)
class _Ps2Key:
    code: int
    type: str


@dataclasses.dataclass(frozen=True)
class _X11Key:
    name: str
    code: int
    shift: bool


@dataclasses.dataclass(frozen=True)
class _KeyMapping:
    web_name: str
    evdev_name: str
    mcu_code: int
    usb_key: _UsbKey
    ps2_key: (_Ps2Key | None)
    at1_code: int
    x11_keys: set[_X11Key]


def _resolve_keysym(name: str) -> int:
    code: (int | None) = None
    for module in [
        Xlib.keysymdef.latin1,
        Xlib.keysymdef.miscellany,
        Xlib.keysymdef.xf86,
        Xlib.keysymdef.xkb,
    ]:
        code = getattr(module, name, None)
        if code is not None:
            break
    assert code is not None, name
    return code


def _parse_x11_names(names: str) -> set[_X11Key]:
    keys: set[_X11Key] = set()
    for name in filter(None, names.split(",")):
        shift = name.startswith("^")
        name = (name[1:] if shift else name)
        code = _resolve_keysym(name)
        keys.add(_X11Key(name, code, shift))
    return keys


def _parse_usb_key(key: str) -> _UsbKey:
    is_modifier = key.startswith("^")
    code = int((key[1:] if is_modifier else key), 16)
    return _UsbKey(code, is_modifier)


def _parse_ps2_key(key: str) -> (_Ps2Key | None):
    if ":" not in key:
        return None
    (code_type, raw_code) = key.split(":")
    return _Ps2Key(
        code=int(raw_code, 16),
        type=code_type,
    )


def _read_keymap_csv(path: str) -> list[_KeyMapping]:
    keymap: list[_KeyMapping] = []
    with open(path) as file:
        for row in csv.DictReader(file):
            if len(row) >= 6:
                keymap.append(_KeyMapping(
                    web_name=row["web_name"],
                    evdev_name=row["evdev_name"],
                    mcu_code=int(row["mcu_code"]),
                    usb_key=_parse_usb_key(row["usb_key"]),
                    ps2_key=_parse_ps2_key(row["ps2_key"]),
                    at1_code=int(row["at1_code"], 16),
                    x11_keys=_parse_x11_names(row["x11_names"] or ""),
                ))
    return keymap


def _render_keymap(keymap: list[_KeyMapping], template_path: str, out_path: str) -> None:
    with open(template_path) as template_file:
        with open(out_path, "w") as out_file:
            template = textwrap.dedent(template_file.read())
            rendered = mako.template.Template(template).render(keymap=keymap)
            out_file.write(rendered)


# =====
def main() -> None:
    # https://developer.mozilla.org/en-US/docs/Web/API/KeyboardEvent/code/code_values
    # https://github.com/NicoHood/HID/blob/master/src/KeyboardLayouts/ImprovedKeylayouts.h
    # https://github.com/Harvie/ps2dev/blob/master/src/ps2dev.h
    # https://gist.github.com/MightyPork/6da26e382a7ad91b5496ee55fdc73db2
    # https://github.com/qemu/keycodemapdb/blob/master/data/keymaps.csv
    # Hut1_12v2.pdf

    # Fields list:
    #   - Web
    #   - Linux/evdev
    #   - MCU code
    #   - USB code (^ for the modifier mask)
    #   - PS/2 key
    #   - AT set1
    #   - X11 keysyms (^ for shift)

    assert len(sys.argv) == 4, f"{sys.argv[0]} <keymap.csv> <template> <out>"

    keymap = _read_keymap_csv(sys.argv[1])
    _render_keymap(keymap, sys.argv[2], sys.argv[3])


# =====
if __name__ == "__main__":
    main()
