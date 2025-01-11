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


import os
import argparse
import pprint
import time

import pyudev

from ..kvmd.switch.device import Device
from ..kvmd.switch.proto import Edid


# =====
def _find_serial_device() -> str:
    ctx = pyudev.Context()
    for device in ctx.list_devices(subsystem="tty"):
        if (
            str(device.properties.get("ID_VENDOR_ID")).upper() == "2E8A"
            and str(device.properties.get("ID_MODEL_ID")).upper() == "1080"
        ):
            path = device.properties["DEVNAME"]
            assert path.startswith("/dev/")
            return path
    return ""


def _wait_boot_device() -> str:
    stop_ts = time.time() + 5
    ctx = pyudev.Context()
    while time.time() < stop_ts:
        for device in ctx.list_devices(subsystem="block", DEVTYPE="partition"):
            if (
                str(device.properties.get("ID_VENDOR_ID")).upper() == "2E8A"
                and str(device.properties.get("ID_MODEL_ID")).upper() == "0003"
            ):
                path = device.properties["DEVNAME"]
                assert path.startswith("/dev/")
                return path
        time.sleep(0.2)
    return ""


def _create_edid(arg: str) -> Edid:
    if arg == "@":
        return Edid.from_data("Empty", None)
    with open(arg) as file:
        return Edid.from_data(os.path.basename(arg), file.read())


# =====
def main() -> None:  # pylint: disable=too-many-statements
    parser = argparse.ArgumentParser()
    parser.add_argument("-d", "--device", default="")
    parser.set_defaults(cmd="")
    subs = parser.add_subparsers()

    def add_command(name: str) -> argparse.ArgumentParser:
        cmd = subs.add_parser(name)
        cmd.set_defaults(cmd=name)
        return cmd

    add_command("poll")

    add_command("state")

    cmd = add_command("bootloader")
    cmd.add_argument("unit", type=int)

    cmd = add_command("reboot")
    cmd.add_argument("unit", type=int)

    cmd = add_command("switch")
    cmd.add_argument("unit", type=int)
    cmd.add_argument("port", type=int, choices=list(range(5)))

    cmd = add_command("beacon")
    cmd.add_argument("unit", type=int)
    cmd.add_argument("port", type=int, choices=list(range(6)))
    cmd.add_argument("on", choices=["on", "off"])

    add_command("leds")

    cmd = add_command("click")
    cmd.add_argument("button", choices=["power", "reset"])
    cmd.add_argument("unit", type=int)
    cmd.add_argument("port", type=int, choices=list(range(4)))
    cmd.add_argument("delay_ms", type=int)

    cmd = add_command("set-edid")
    cmd.add_argument("unit", type=int)
    cmd.add_argument("port", type=int, choices=list(range(4)))
    cmd.add_argument("edid", type=_create_edid)

    opts = parser.parse_args()

    if not opts.device:
        opts.device = _find_serial_device()

    if opts.cmd == "bootloader" and opts.unit == 0:
        if opts.device:
            with Device(opts.device) as device:
                device.request_reboot(opts.unit, bootloader=True)
        found = _wait_boot_device()
        if found:
            print(found)
            raise SystemExit()
        raise SystemExit("Error: No switch found")

    if not opts.device:
        raise SystemExit("Error: No switch found")

    with Device(opts.device) as device:
        wait_rid: (int | None) = None
        match opts.cmd:
            case "poll":
                device.request_state()
                device.request_atx_leds()
            case "state":
                wait_rid = device.request_state()
            case "bootloader" | "reboot":
                device.request_reboot(opts.unit, (opts.cmd == "bootloader"))
                raise SystemExit()
            case "switch":
                wait_rid = device.request_switch(opts.unit, opts.port)
            case "leds":
                wait_rid = device.request_atx_leds()
            case "click":
                match opts.button:
                    case "power":
                        wait_rid = device.request_atx_cp(opts.unit, opts.port, opts.delay_ms)
                    case "reset":
                        wait_rid = device.request_atx_cr(opts.unit, opts.port, opts.delay_ms)
            case "beacon":
                wait_rid = device.request_beacon(opts.unit, opts.port, (opts.on == "on"))
            case "set-edid":
                wait_rid = device.request_set_edid(opts.unit, opts.port, opts.edid)

        error_ts = time.monotonic() + 1
        while True:
            for resp in device.read_all():
                pprint.pprint((int(time.time()), resp))
                print()
                if resp.header.rid == wait_rid:
                    raise SystemExit()
            if wait_rid is not None and time.monotonic() > error_ts:
                raise SystemExit("No answer from unit")
