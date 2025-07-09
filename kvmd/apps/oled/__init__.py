#!/usr/bin/env python3
# ========================================================================== #
#                                                                            #
#    KVMD-OLED - A small OLED daemon for PiKVM.                              #
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
import os
import signal
import itertools
import logging
import time

import usb.core

from luma.core import cmdline as luma_cmdline

from PIL import ImageFont

from .screen import Screen
from .sensors import Sensors


# =====
_logger = logging.getLogger("oled")


# =====
def _detect_geometry() -> dict:
    with open("/proc/device-tree/model") as file:
        is_cm4 = ("Compute Module 4" in file.read())
    has_usb = bool(list(usb.core.find(find_all=True)))
    if is_cm4 and has_usb:
        return {"height": 64, "rotate": 2}
    return {"height": 32, "rotate": 0}


def _get_data_path(subdir: str, name: str) -> str:
    if not name.startswith("@"):
        return name  # Just a regular system path
    name = name[1:]
    module_path = sys.modules[__name__].__file__
    assert module_path is not None
    return os.path.join(os.path.dirname(module_path), subdir, name)


# =====
def main() -> None:  # pylint: disable=too-many-locals,too-many-branches,too-many-statements
    logging.basicConfig(level=logging.INFO, format="%(message)s")
    logging.getLogger("PIL").setLevel(logging.ERROR)

    parser = luma_cmdline.create_parser(description="Display FQDN and IP on the OLED")
    parser.set_defaults(**_detect_geometry())

    parser.add_argument("--font", default="@ProggySquare.ttf", type=(lambda arg: _get_data_path("fonts", arg)), help="Font path")
    parser.add_argument("--font-size", default=16, type=int, help="Font size")
    parser.add_argument("--font-spacing", default=2, type=int, help="Font line spacing")
    parser.add_argument("--offset-x", default=0, type=int, help="Horizontal offset")
    parser.add_argument("--offset-y", default=0, type=int, help="Vertical offset")
    parser.add_argument("--interval", default=5, type=int, help="Screens interval")
    parser.add_argument("--image", default="", type=(lambda arg: _get_data_path("pics", arg)), help="Display some image, wait a single interval and exit")
    parser.add_argument("--text", default="", help="Display some text, wait a single interval and exit")
    parser.add_argument("--pipe", action="store_true", help="Read and display lines from stdin until EOF, wait a single interval and exit")
    parser.add_argument("--fill", action="store_true", help="Fill the display with 0xFF")
    parser.add_argument("--clear-on-exit", action="store_true", help="Clear display on exit")
    parser.add_argument("--contrast", default=64, type=int, help="Set OLED contrast, values from 0 to 255")
    parser.add_argument("--fahrenheit", action="store_true", help="Display temperature in Fahrenheit instead of Celsius")
    options = parser.parse_args(sys.argv[1:])
    if options.config:
        config = luma_cmdline.load_config(options.config)
        options = parser.parse_args(config + sys.argv[1:])

    device = luma_cmdline.create_device(options)
    device.cleanup = (lambda _: None)
    screen = Screen(
        device=device,
        font=ImageFont.truetype(options.font, options.font_size),
        font_spacing=options.font_spacing,
        offset=(options.offset_x, options.offset_y),
    )

    if options.display not in luma_cmdline.get_display_types()["emulator"]:
        _logger.info("Iface: %s", options.interface)
    _logger.info("Display: %s", options.display)
    _logger.info("Size: %dx%d", device.width, device.height)
    options.contrast = min(max(options.contrast, 0), 255)
    _logger.info("Contrast: %d", options.contrast)
    device.contrast(options.contrast)

    try:
        if options.image:
            screen.draw_image(options.image)
            time.sleep(options.interval)

        elif options.text:
            screen.draw_text(options.text.replace("\\n", "\n"))
            time.sleep(options.interval)

        elif options.pipe:
            text = ""
            for line in sys.stdin:
                text += line
                if "\0" in text:
                    screen.draw_text(text.replace("\0", ""))
                    text = ""
            time.sleep(options.interval)

        elif options.fill:
            screen.draw_white()

        else:
            stop_reason: (str | None) = None

            def sigusr_handler(signum: int, _) -> None:  # type: ignore
                nonlocal stop_reason
                if signum in (signal.SIGINT, signal.SIGTERM):
                    stop_reason = ""
                elif signum == signal.SIGUSR1:
                    stop_reason = "Rebooting...\nPlease wait"
                elif signum == signal.SIGUSR2:
                    stop_reason = "Halted"

            for signum in [signal.SIGTERM, signal.SIGINT, signal.SIGUSR1, signal.SIGUSR2]:
                signal.signal(signum, sigusr_handler)

            hb = itertools.cycle(r"/-\|")  # Heartbeat
            swim = 0

            def draw(text: str) -> None:
                nonlocal swim
                count = 0
                while (count < max(options.interval, 1) * 2) and stop_reason is None:
                    screen.draw_text(
                        text=text.replace("__hb__", next(hb)),
                        offset_x=(3 if swim < 0 else 0),
                    )
                    count += 1
                    if swim >= 1200:
                        swim = -1200
                    else:
                        swim += 1
                    time.sleep(0.5)

            sensors = Sensors(options.fahrenheit)

            if device.height >= 64:
                while stop_reason is None:
                    text = "{fqdn}\n{ip}\niface: {iface}\ntemp: {temp}\ncpu: {cpu} mem: {mem}\n(__hb__) {uptime}"
                    draw(sensors.render(text))
            else:
                summary = True
                while stop_reason is None:
                    if summary:
                        text = "{fqdn}\n(__hb__) {uptime}\ntemp: {temp}"
                    else:
                        text = "{ip}\n(__hb__) iface: {iface}\ncpu: {cpu} mem: {mem}"
                    draw(sensors.render(text))
                    summary = (not summary)

            if stop_reason is not None:
                if len(stop_reason) > 0:
                    options.clear_on_exit = False
                    screen.draw_text(stop_reason)
                while len(stop_reason) > 0:
                    time.sleep(0.1)

    except (SystemExit, KeyboardInterrupt):
        pass

    if options.clear_on_exit:
        screen.draw_text("")
