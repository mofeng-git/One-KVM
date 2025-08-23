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
import os
import subprocess
import argparse
import time

from typing import Callable

from ...validators.basic import valid_bool
from ...validators.basic import valid_int_f0

from ...edid import EdidNoBlockError
from ...edid import Edid

# from .. import init


# =====
def _format_bool(value: bool) -> str:
    return ("yes" if value else "no")


def _make_format_hex(size: int) -> Callable[[int], str]:
    return (lambda value: ("0x{:0%dX} ({})" % (size * 2)).format(value, value))


def _print_edid(edid: Edid) -> None:
    for (key, get, fmt) in [
        ("Manufacturer ID:", edid.get_mfc_id,         str),
        ("Product ID:     ", edid.get_product_id,     _make_format_hex(2)),
        ("Serial number:  ", edid.get_serial,         _make_format_hex(4)),
        ("Monitor name:   ", edid.get_monitor_name,   str),
        ("Monitor serial: ", edid.get_monitor_serial, str),
        ("Audio:          ", edid.get_audio,          _format_bool),
    ]:
        try:
            print(key, fmt(get()), file=sys.stderr)  # type: ignore
        except EdidNoBlockError:
            pass


def _find_out2_edid_path() -> str:
    card = os.path.basename(os.readlink("/dev/dri/by-path/platform-gpu-card"))
    path = f"/sys/devices/platform/gpu/drm/{card}/{card}-HDMI-A-2"
    with open(os.path.join(path, "status")) as file:
        if file.read().startswith("d"):
            raise SystemExit("No display found")
    return os.path.join(path, "edid")


def _adopt_out2_ids(dest: Edid) -> None:
    src = Edid.from_file(_find_out2_edid_path())
    dest.set_monitor_name(src.get_monitor_name())
    try:
        dest.get_monitor_serial()
    except EdidNoBlockError:
        pass
    else:
        try:
            ser = src.get_monitor_serial()
        except EdidNoBlockError:
            ser = "{:08X}".format(src.get_serial())
        dest.set_monitor_serial(ser)
    dest.set_mfc_id(src.get_mfc_id())
    dest.set_product_id(src.get_product_id())
    dest.set_serial(src.get_serial())


# =====
def main(argv: (list[str] | None)=None) -> None:  # pylint: disable=too-many-branches,too-many-statements
    # (parent_parser, argv, _) = init(
    #     add_help=False,
    #     argv=argv,
    # )
    if argv is None:
        argv = sys.argv
    parser = argparse.ArgumentParser(
        prog="kvmd-edidconf",
        description="A simple and primitive KVMD EDID editor",
        # parents=[parent_parser],
    )

    lane2 = ["v0", "v1", "v2", "v3"]
    lane4 = ["v4mini", "v4plus"]
    presets = lane2 + lane4 + [f"{name}.1080p-by-default" for name in lane2] + [f"{name}.no-1920x1200" for name in lane4]

    parser.add_argument("-f", "--edid", dest="edid_path", default="/etc/kvmd/tc358743-edid.hex",
                        help="The hex/bin EDID file path", metavar="<file>")
    parser.add_argument("--export-hex",
                        help="Export [--edid] file to the new file as a hex text", metavar="<file>")
    parser.add_argument("--export-bin",
                        help="Export [--edid] file to the new file as a bin data", metavar="<file>")
    parser.add_argument("--import", dest="imp",
                        help="Import the specified bin/hex EDID to the [--edid] file as a hex text", metavar="<file>")
    parser.add_argument("--import-preset", choices=presets,
                        help="Restore default EDID or choose the preset", metavar=f"{{ {' | '.join(presets)} }}",)
    parser.add_argument("--import-display-ids", action="store_true",
                        help="On PiKVM V4, import and adopt IDs from a physical display connected to the OUT2 port")
    parser.add_argument("--import-display", action="store_true",
                        help="On PiKVM V4, import full EDID from a physical display connected to the OUT2 port")
    parser.add_argument("--set-audio", type=valid_bool,
                        help="Enable or disable audio", metavar="<yes|no>")
    parser.add_argument("--set-mfc-id",
                        help="Set manufacturer ID (https://uefi.org/pnp_id_list)", metavar="<ABC>")
    parser.add_argument("--set-product-id", type=valid_int_f0,
                        help="Set product ID (decimal)", metavar="<uint>")
    parser.add_argument("--set-serial", type=valid_int_f0,
                        help="Set serial number (decimal)", metavar="<uint>")
    parser.add_argument("--set-monitor-name",
                        help="Set monitor name in DTD block (ASCII, max 13 characters)", metavar="<str>")
    parser.add_argument("--set-monitor-serial",
                        help="Set monitor serial in DTD block if exists (ASCII, max 13 characters)", metavar="<str>")
    parser.add_argument("--clear", action="store_true",
                        help="Clear the EDID in the [--device]")
    parser.add_argument("--apply", action="store_true",
                        help="Apply [--edid] on the [--device]")
    parser.add_argument("--device", dest="device_path", default="/dev/kvmd-video",
                        help="The video device", metavar="<device>")
    parser.add_argument("--presets", dest="presets_path", default="/usr/share/kvmd/configs.default/kvmd/edid",
                        help="Presets directory", metavar="<dir>")
    options = parser.parse_args(argv[1:])

    base: (Edid | None) = None
    if options.import_preset:
        imp = options.import_preset
        if "." in imp:
            (base_name, imp) = imp.split(".", 1)  # v3.1080p-by-default
            base = Edid.from_file(os.path.join(options.presets_path, f"{base_name}.hex"))
            imp = f"_{imp}"
        options.imp = os.path.join(options.presets_path, f"{imp}.hex")

    if options.import_display:
        options.imp = _find_out2_edid_path()

    orig_edid_path = options.edid_path
    if options.imp:
        options.export_hex = options.edid_path
        options.edid_path = options.imp

    edid = Edid.from_file(options.edid_path)
    changed = False

    if options.import_display_ids:
        _adopt_out2_ids(edid)
        changed = True

    for cmd in dir(Edid):
        if cmd.startswith("set_"):
            value = getattr(options, cmd)
            if value is None and base is not None:
                try:
                    value = getattr(base, cmd.replace("set_", "get_"))()
                except EdidNoBlockError:
                    pass
            if value is not None:
                getattr(edid, cmd)(value)
                changed = True

    if options.export_hex is not None:
        edid.write_hex(options.export_hex)
    elif options.export_bin is not None:
        edid.write_bin(options.export_bin)
    elif changed:
        edid.write_hex(options.edid_path)

    _print_edid(edid)

    try:
        if options.clear:
            subprocess.run([
                "/usr/bin/v4l2-ctl",
                f"--device={options.device_path}",
                "--clear-edid",
            ], stdout=sys.stderr, check=True)
            if options.apply:
                time.sleep(1)
        if options.apply:
            subprocess.run([
                "/usr/bin/v4l2-ctl",
                f"--device={options.device_path}",
                f"--set-edid=file={orig_edid_path}",
                "--info-edid",
            ], stdout=sys.stderr, check=True)
    except subprocess.CalledProcessError as ex:
        raise SystemExit(str(ex))
