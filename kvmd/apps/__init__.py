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
import functools
import argparse
import logging
import logging.config

import pygments
import pygments.lexers.data
import pygments.formatters

from .. import tools

from ..plugins import UnknownPluginError
from ..plugins.auth import get_auth_service_class
from ..plugins.hid import get_hid_class
from ..plugins.atx import get_atx_class
from ..plugins.msd import get_msd_class

from ..plugins.ugpio import UserGpioModes
from ..plugins.ugpio import BaseUserGpioDriver
from ..plugins.ugpio import get_ugpio_driver_class

from ..yamlconf import ConfigError
from ..yamlconf import manual_validated
from ..yamlconf import make_config
from ..yamlconf import Section
from ..yamlconf import Option
from ..yamlconf import build_raw_from_options
from ..yamlconf.dumper import make_config_dump
from ..yamlconf.loader import load_yaml_file
from ..yamlconf.merger import yaml_merge

from ..validators.basic import valid_stripped_string
from ..validators.basic import valid_stripped_string_not_empty
from ..validators.basic import valid_bool
from ..validators.basic import valid_number
from ..validators.basic import valid_int_f0
from ..validators.basic import valid_int_f1
from ..validators.basic import valid_float_f0
from ..validators.basic import valid_float_f01
from ..validators.basic import valid_string_list

from ..validators.auth import valid_user
from ..validators.auth import valid_users_list

from ..validators.os import valid_abs_path
from ..validators.os import valid_abs_file
from ..validators.os import valid_abs_dir
from ..validators.os import valid_unix_mode
from ..validators.os import valid_options
from ..validators.os import valid_command

from ..validators.net import valid_ip_or_host
from ..validators.net import valid_net
from ..validators.net import valid_port
from ..validators.net import valid_ports_list
from ..validators.net import valid_mac
from ..validators.net import valid_ssl_ciphers

from ..validators.hid import valid_hid_key
from ..validators.hid import valid_hid_mouse_output
from ..validators.hid import valid_hid_mouse_move

from ..validators.kvm import valid_stream_quality
from ..validators.kvm import valid_stream_fps
from ..validators.kvm import valid_stream_resolution
from ..validators.kvm import valid_stream_h264_bitrate
from ..validators.kvm import valid_stream_h264_gop

from ..validators.ugpio import valid_ugpio_driver
from ..validators.ugpio import valid_ugpio_channel
from ..validators.ugpio import valid_ugpio_mode
from ..validators.ugpio import valid_ugpio_view_title
from ..validators.ugpio import valid_ugpio_view_table

from ..validators.hw import valid_tty_speed
from ..validators.hw import valid_otg_gadget
from ..validators.hw import valid_otg_id
from ..validators.hw import valid_otg_ethernet


# =====
def init(
    prog: (str | None)=None,
    description: (str | None)=None,
    add_help: bool=True,
    check_run: bool=False,
    cli_logging: bool=False,
    argv: (list[str] | None)=None,
    **load: bool,
) -> tuple[argparse.ArgumentParser, list[str], Section]:

    argv = (argv or sys.argv)
    assert len(argv) > 0

    parser = argparse.ArgumentParser(
        prog=(prog or argv[0]),
        description=description,
        add_help=add_help,
        formatter_class=argparse.ArgumentDefaultsHelpFormatter,
    )
    parser.add_argument("-c", "--config", default="/etc/kvmd/main.yaml", type=valid_abs_file,
                        help="Set config file path", metavar="<file>")
    parser.add_argument("-o", "--set-options", default=[], nargs="+",
                        help="Override config options list (like sec/sub/opt=value)", metavar="<k=v>",)
    parser.add_argument("-m", "--dump-config", action="store_true",
                        help="View current configuration (include all overrides)")
    if check_run:
        parser.add_argument("--run", dest="run", action="store_true",
                            help="Run the service")
    (options, remaining) = parser.parse_known_args(argv)

    if options.dump_config:
        _dump_config(_init_config(
            config_path=options.config,
            override_options=options.set_options,
            load_auth=True,
            load_hid=True,
            load_atx=True,
            load_msd=True,
            load_gpio=True,
        ))
        raise SystemExit()
    config = _init_config(options.config, options.set_options, **load)

    logging.captureWarnings(True)
    logging.config.dictConfig(config.logging)
    if cli_logging:
        logging.getLogger().handlers[0].setFormatter(logging.Formatter(
            "-- {levelname:>7} -- {message}",
            style="{",
        ))

    if check_run and not options.run:
        raise SystemExit(
            "To prevent accidental startup, you must specify the --run option to start.\n"
            "Try the --help option to find out what this service does.\n"
            "Make sure you understand exactly what you are doing!"
        )

    return (parser, remaining, config)


# =====
def _init_config(config_path: str, override_options: list[str], **load_flags: bool) -> Section:
    config_path = os.path.expanduser(config_path)
    try:
        raw_config: dict = load_yaml_file(config_path)
    except Exception as ex:
        raise SystemExit(f"ConfigError: Can't read config file {config_path!r}:\n{tools.efmt(ex)}")
    if not isinstance(raw_config, dict):
        raise SystemExit(f"ConfigError: Top-level of the file {config_path!r} must be a dictionary")

    scheme = _get_config_scheme()
    try:
        yaml_merge(raw_config, (raw_config.pop("override", {}) or {}))
        yaml_merge(raw_config, build_raw_from_options(override_options), "raw CLI options")
        _patch_raw(raw_config)
        config = make_config(raw_config, scheme)

        if _patch_dynamic(raw_config, config, scheme, **load_flags):
            config = make_config(raw_config, scheme)

        return config
    except (ConfigError, UnknownPluginError) as ex:
        raise SystemExit(f"ConfigError: {ex}")


def _patch_raw(raw_config: dict) -> None:  # pylint: disable=too-many-branches
    if isinstance(raw_config.get("otg"), dict):
        for (old, new) in [
            ("msd", "msd"),
            ("acm", "serial"),
            ("drives", "drives"),
        ]:
            if old in raw_config["otg"]:
                if not isinstance(raw_config["otg"].get("devices"), dict):
                    raw_config["otg"]["devices"] = {}
                raw_config["otg"]["devices"][new] = raw_config["otg"].pop(old)

    if isinstance(raw_config.get("kvmd"), dict) and isinstance(raw_config["kvmd"].get("wol"), dict):
        if not isinstance(raw_config["kvmd"].get("gpio"), dict):
            raw_config["kvmd"]["gpio"] = {}
        for section in ["drivers", "scheme"]:
            if not isinstance(raw_config["kvmd"]["gpio"].get(section), dict):
                raw_config["kvmd"]["gpio"][section] = {}
        raw_config["kvmd"]["gpio"]["drivers"]["__wol__"] = {
            "type": "wol",
            **raw_config["kvmd"].pop("wol"),
        }
        raw_config["kvmd"]["gpio"]["scheme"]["__wol__"] = {
            "driver": "__wol__",
            "pin": 0,
            "mode": "output",
            "switch": False,
        }

    if isinstance(raw_config.get("kvmd"), dict) and isinstance(raw_config["kvmd"].get("streamer"), dict):
        streamer_config = raw_config["kvmd"]["streamer"]

        desired_fps = streamer_config.get("desired_fps")
        if desired_fps is not None and not isinstance(desired_fps, dict):
            streamer_config["desired_fps"] = {"default": desired_fps}

        max_fps = streamer_config.get("max_fps")
        if max_fps is not None:
            if not isinstance(streamer_config.get("desired_fps"), dict):
                streamer_config["desired_fps"] = {}
            streamer_config["desired_fps"]["max"] = max_fps
            del streamer_config["max_fps"]

        resolution = streamer_config.get("resolution")
        if resolution is not None and not isinstance(resolution, dict):
            streamer_config["resolution"] = {"default": resolution}

        available_resolutions = streamer_config.get("available_resolutions")
        if available_resolutions is not None:
            if not isinstance(streamer_config.get("resolution"), dict):
                streamer_config["resolution"] = {}
            streamer_config["resolution"]["available"] = available_resolutions
            del streamer_config["available_resolutions"]


def _patch_dynamic(  # pylint: disable=too-many-locals
    raw_config: dict,
    config: Section,
    scheme: dict,
    load_auth: bool=False,
    load_hid: bool=False,
    load_atx: bool=False,
    load_msd: bool=False,
    load_gpio: bool=False,
) -> bool:

    rebuild = False

    if load_auth:
        scheme["kvmd"]["auth"]["internal"].update(get_auth_service_class(config.kvmd.auth.internal.type).get_plugin_options())
        if config.kvmd.auth.external.type:
            scheme["kvmd"]["auth"]["external"].update(get_auth_service_class(config.kvmd.auth.external.type).get_plugin_options())
        rebuild = True

    for (load, section, get_class) in [
        (load_hid, "hid", get_hid_class),
        (load_atx, "atx", get_atx_class),
        (load_msd, "msd", get_msd_class),
    ]:
        if load:
            scheme["kvmd"][section].update(get_class(getattr(config.kvmd, section).type).get_plugin_options())
            rebuild = True

    if load_gpio:
        driver: str
        drivers: dict[str, type[BaseUserGpioDriver]] = {}  # Name to drivers
        for (driver, params) in {  # type: ignore
            "__gpio__": {},
            **tools.rget(raw_config, "kvmd", "gpio", "drivers"),
        }.items():
            with manual_validated(driver, "kvmd", "gpio", "drivers", "<key>"):
                driver = valid_ugpio_driver(driver)

            driver_type = valid_stripped_string_not_empty(params.get("type", "gpio"))
            driver_class = get_ugpio_driver_class(driver_type)
            drivers[driver] = driver_class
            scheme["kvmd"]["gpio"]["drivers"][driver] = {
                "type": Option(driver_type, type=valid_stripped_string_not_empty),
                **driver_class.get_plugin_options()
            }

        path = ("kvmd", "gpio", "scheme")
        for (channel, params) in tools.rget(raw_config, *path).items():
            with manual_validated(channel, *path, "<key>"):
                channel = valid_ugpio_channel(channel)

            driver = params.get("driver", "__gpio__")
            with manual_validated(driver, *path, channel, "driver"):
                driver = valid_ugpio_driver(driver, set(drivers))

            mode: str = params.get("mode", "")
            with manual_validated(mode, *path, channel, "mode"):
                mode = valid_ugpio_mode(mode, drivers[driver].get_modes())

            if params.get("pulse") == False:  # noqa: E712  # pylint: disable=singleton-comparison
                params["pulse"] = {"delay": 0}

            scheme["kvmd"]["gpio"]["scheme"][channel] = {
                "driver":   Option("__gpio__", type=functools.partial(valid_ugpio_driver, variants=set(drivers))),
                "pin":      Option(None,       type=drivers[driver].get_pin_validator()),
                "mode":     Option("",         type=functools.partial(valid_ugpio_mode, variants=drivers[driver].get_modes())),
                "inverted": Option(False,      type=valid_bool),
                **({
                    "busy_delay": Option(0.2,   type=valid_float_f01),
                    "initial":    Option(False, type=(lambda arg: (valid_bool(arg) if arg is not None else None))),
                    "switch":     Option(True,  type=valid_bool),
                    "pulse": {  # type: ignore
                        "delay":     Option(0.1, type=valid_float_f0),
                        "min_delay": Option(0.1, type=valid_float_f01),
                        "max_delay": Option(0.1, type=valid_float_f01),
                    },
                } if mode == UserGpioModes.OUTPUT else {  # input
                    "debounce": Option(0.1, type=valid_float_f0),
                })
            }

        rebuild = True

    return rebuild


def _dump_config(config: Section) -> None:
    dump = make_config_dump(config)
    if sys.stdout.isatty():
        dump = pygments.highlight(
            dump,
            pygments.lexers.data.YamlLexer(),
            pygments.formatters.TerminalFormatter(bg="dark"),  # pylint: disable=no-member
        )
    print(dump)


def _get_config_scheme() -> dict:
    return {
        "logging": Option({}),

        "kvmd": {
            "server": {
                "unix":              Option("/run/kvmd/kvmd.sock", type=valid_abs_path, unpack_as="unix_path"),
                "unix_rm":           Option(True,  type=valid_bool),
                "unix_mode":         Option(0o660, type=valid_unix_mode),
                "heartbeat":         Option(15.0,  type=valid_float_f01),
                "access_log_format": Option("[%P / %{X-Real-IP}i] '%r' => %s; size=%b ---"
                                            " referer='%{Referer}i'; user_agent='%{User-Agent}i'"),
            },

            "auth": {
                "enabled": Option(True, type=valid_bool),

                "internal": {
                    "type":        Option("htpasswd"),
                    "force_users": Option([], type=valid_users_list),
                    # Dynamic content
                },

                "external": {
                    "type": Option("", type=valid_stripped_string),
                    # Dynamic content
                },

                "totp": {
                    "valid_window": Option(1, type=functools.partial(valid_number, min=0, max=5)),
                    "secret": {
                        "file": Option("/etc/kvmd/totp.secret", type=valid_abs_path, if_empty=""),
                    },
                },
            },

            "info": {  # Accessed via global config, see kvmd/info for details
                "meta":   Option("/etc/kvmd/meta.yaml",    type=valid_abs_file),
                "extras": Option("/usr/share/kvmd/extras", type=valid_abs_dir),
                "hw": {
                    "platform":      Option("/usr/share/kvmd/platform", type=valid_abs_file, unpack_as="platform_path"),
                    "vcgencmd_cmd":  Option(["/usr/bin/vcgencmd"], type=valid_command),
                    "ignore_past":   Option(False, type=valid_bool),
                    "state_poll":    Option(5.0,   type=valid_float_f01),
                },
                "fan": {
                    "daemon":     Option("kvmd-fan", type=valid_stripped_string),
                    "unix":       Option("",  type=valid_abs_path, if_empty="", unpack_as="unix_path"),
                    "timeout":    Option(5.0, type=valid_float_f01),
                    "state_poll": Option(5.0, type=valid_float_f01),
                },
            },

            "log_reader": {
                "enabled": Option(True, type=valid_bool),
            },

            "prometheus": {
                "auth": {
                    "enabled": Option(True, type=valid_bool),
                },
            },

            "hid": {
                "type": Option("", type=valid_stripped_string_not_empty),
                "keymap": Option("/usr/share/kvmd/keymaps/en-us", type=valid_abs_file),
                # Dynamic content
            },

            "atx": {
                "type": Option("", type=valid_stripped_string_not_empty),
                # Dynamic content
            },

            "msd": {
                "type": Option("", type=valid_stripped_string_not_empty),
                # Dynamic content
            },

            "streamer": {
                "forever": Option(False, type=valid_bool),

                "reset_delay":    Option(1.0,  type=valid_float_f0),
                "shutdown_delay": Option(10.0, type=valid_float_f01),
                "state_poll":     Option(1.0,  type=valid_float_f01),

                "quality": Option(80, type=valid_stream_quality, if_empty=0),

                "resolution": {
                    "default":   Option("", type=valid_stream_resolution, if_empty="", unpack_as="resolution"),
                    "available": Option(
                        [],
                        type=functools.partial(valid_string_list, subval=valid_stream_resolution),
                        unpack_as="available_resolutions",
                    ),
                },

                "desired_fps": {
                    "default": Option(40, type=valid_stream_fps, unpack_as="desired_fps"),
                    "min":     Option(0,  type=valid_stream_fps, unpack_as="desired_fps_min"),
                    "max":     Option(70, type=valid_stream_fps, unpack_as="desired_fps_max"),
                },

                "h264_bitrate": {
                    "default": Option(0,     type=valid_stream_h264_bitrate, if_empty=0, unpack_as="h264_bitrate"),
                    "min":     Option(25,    type=valid_stream_h264_bitrate, unpack_as="h264_bitrate_min"),
                    "max":     Option(20000, type=valid_stream_h264_bitrate, unpack_as="h264_bitrate_max"),
                },

                "h264_gop": {
                    "default": Option(30, type=valid_stream_h264_gop, unpack_as="h264_gop"),
                    "min":     Option(0,  type=valid_stream_h264_gop, unpack_as="h264_gop_min"),
                    "max":     Option(60, type=valid_stream_h264_gop, unpack_as="h264_gop_max"),
                },

                "unix":    Option("/run/kvmd/ustreamer.sock", type=valid_abs_path, unpack_as="unix_path"),
                "timeout": Option(2.0, type=valid_float_f01),
                "snapshot_timeout": Option(1.0, type=valid_float_f01),  # error_delay * 3 + 1

                "process_name_prefix": Option("kvmd/streamer"),

                "pre_start_cmd":        Option(["/bin/true", "pre-start"], type=valid_command),
                "pre_start_cmd_remove": Option([], type=valid_options),
                "pre_start_cmd_append": Option([], type=valid_options),

                "cmd":        Option(["/bin/true"], type=valid_command),
                "cmd_remove": Option([], type=valid_options),
                "cmd_append": Option([], type=valid_options),

                "post_stop_cmd":        Option(["/bin/true", "post-stop"], type=valid_command),
                "post_stop_cmd_remove": Option([], type=valid_options),
                "post_stop_cmd_append": Option([], type=valid_options),
            },

            "ocr": {
                "langs":    Option(["eng"], type=valid_string_list, unpack_as="default_langs"),
                "tessdata": Option("/usr/share/tessdata", type=valid_stripped_string_not_empty, unpack_as="data_dir_path")
            },

            "snapshot": {
                "idle_interval": Option(0.0, type=valid_float_f0),
                "live_interval": Option(0.0, type=valid_float_f0),

                "wakeup_key":  Option("", type=valid_hid_key, if_empty=""),
                "wakeup_move": Option(0,  type=valid_hid_mouse_move),

                "online_delay":  Option(5.0, type=valid_float_f0),
                "retries":       Option(10,  type=valid_int_f1),
                "retries_delay": Option(3.0, type=valid_float_f01),
            },

            "gpio": {
                "state_poll": Option(0.1, type=valid_float_f01),
                "drivers": {},  # Dynamic content
                "scheme": {},  # Dymanic content
                "view": {
                    "header": {
                        "title": Option("GPIO", type=valid_ugpio_view_title),
                    },
                    "table": Option([], type=valid_ugpio_view_table),
                },
            },

            "switch": {
                "device":       Option("/dev/kvmd-switch", type=valid_abs_path, unpack_as="device_path"),
                "default_edid": Option("/etc/kvmd/switch-edid.hex", type=valid_abs_path, unpack_as="default_edid_path"),
            },
        },

        "media": {
            "server": {
                "unix":              Option("/run/kvmd/media.sock", type=valid_abs_path, unpack_as="unix_path"),
                "unix_rm":           Option(True,  type=valid_bool),
                "unix_mode":         Option(0o660, type=valid_unix_mode),
                "heartbeat":         Option(15.0,  type=valid_float_f01),
                "access_log_format": Option("[%P / %{X-Real-IP}i] '%r' => %s; size=%b ---"
                                            " referer='%{Referer}i'; user_agent='%{User-Agent}i'"),
            },

            "memsink": {
                "jpeg": {
                    "sink":             Option("",  unpack_as="obj"),
                    "lock_timeout":     Option(1.0, type=valid_float_f01),
                    "wait_timeout":     Option(1.0, type=valid_float_f01),
                    "drop_same_frames": Option(0.0, type=valid_float_f0),
                },
                "h264": {
                    "sink":             Option("",  unpack_as="obj"),
                    "lock_timeout":     Option(1.0, type=valid_float_f01),
                    "wait_timeout":     Option(1.0, type=valid_float_f01),
                    "drop_same_frames": Option(0.0, type=valid_float_f0),
                },
            },
        },

        "pst": {
            "server": {
                "unix":              Option("/run/kvmd/pst.sock", type=valid_abs_path, unpack_as="unix_path"),
                "unix_rm":           Option(True,  type=valid_bool),
                "unix_mode":         Option(0o660, type=valid_unix_mode),
                "heartbeat":         Option(15.0,  type=valid_float_f01),
                "access_log_format": Option("[%P / %{X-Real-IP}i] '%r' => %s; size=%b ---"
                                            " referer='%{Referer}i'; user_agent='%{User-Agent}i'"),
            },

            "ro_retries_delay": Option(10.0, type=valid_float_f01),
            "ro_cleanup_delay": Option(3.0,  type=valid_float_f01),

            "remount_cmd": Option([
                "/usr/bin/sudo", "--non-interactive",
                "/usr/bin/kvmd-helper-pst-remount", "{mode}",
            ], type=valid_command),
        },

        "otg": {
            "vendor_id":      Option(0x1D6B, type=valid_otg_id),  # Linux Foundation
            "product_id":     Option(0x0104, type=valid_otg_id),  # Multifunction Composite Gadget
            "manufacturer":   Option("PiKVM", type=valid_stripped_string),
            "product":        Option("Composite KVM Device", type=valid_stripped_string),
            "serial":         Option("CAFEBABE", type=valid_stripped_string, if_none=None),
            "device_version": Option(-1,     type=functools.partial(valid_number, min=-1, max=0xFFFF)),
            "usb_version":    Option(0x0200, type=valid_otg_id),
            "max_power":      Option(250,    type=functools.partial(valid_number, min=50, max=500)),
            "remote_wakeup":  Option(True,   type=valid_bool),

            "gadget":     Option("kvmd", type=valid_otg_gadget),
            "config":     Option("PiKVM device", type=valid_stripped_string_not_empty),
            "udc":        Option("",     type=valid_stripped_string),
            "endpoints":  Option(9,      type=valid_int_f0),
            "init_delay": Option(3.0,    type=valid_float_f01),

            "user": Option("kvmd", type=valid_user),
            "meta": Option("/run/kvmd/otg", type=valid_abs_path),

            "devices": {
                "hid": {
                    "keyboard": {
                        "start": Option(True, type=valid_bool),
                    },
                    "mouse": {
                        "start": Option(True, type=valid_bool),
                    },
                    "mouse_alt": {
                        "start": Option(True, type=valid_bool),
                    },
                },

                "msd": {
                    "start": Option(True, type=valid_bool),
                    "default": {
                        "stall":     Option(False, type=valid_bool),
                        "cdrom":     Option(True,  type=valid_bool),
                        "rw":        Option(False, type=valid_bool),
                        "removable": Option(True,  type=valid_bool),
                        "fua":       Option(True,  type=valid_bool),
                    },
                },

                "serial": {
                    "enabled": Option(False, type=valid_bool),
                    "start":   Option(True,  type=valid_bool),
                },

                "ethernet": {
                    "enabled":  Option(False, type=valid_bool),
                    "start":    Option(True,  type=valid_bool),
                    "driver":   Option("ecm", type=valid_otg_ethernet),
                    "host_mac": Option("",    type=valid_mac, if_empty=""),
                    "kvm_mac":  Option("",    type=valid_mac, if_empty=""),
                },

                "drives": {
                    "enabled": Option(False, type=valid_bool),
                    "start":   Option(True,  type=valid_bool),
                    "count":   Option(1,     type=valid_int_f1),
                    "default": {
                        "stall":     Option(False, type=valid_bool),
                        "cdrom":     Option(False, type=valid_bool),
                        "rw":        Option(True,  type=valid_bool),
                        "removable": Option(True,  type=valid_bool),
                        "fua":       Option(True,  type=valid_bool),
                    },
                },
            },
        },

        "otgnet": {
            "iface": {
                "net":    Option("172.30.30.0/24", type=functools.partial(valid_net, v6=False)),
                "ip_cmd": Option(["/usr/bin/ip"],  type=valid_command),
            },

            "firewall": {
                "allow_icmp":    Option(True, type=valid_bool),
                "allow_tcp":     Option([],   type=valid_ports_list),
                "allow_udp":     Option([67], type=valid_ports_list),
                "forward_iface": Option("",   type=valid_stripped_string),
                "iptables_cmd":  Option(["/usr/sbin/iptables", "--wait=5"], type=valid_command),
            },

            "commands": {
                "pre_start_cmd":        Option(["/bin/true", "pre-start"], type=valid_command),
                "pre_start_cmd_remove": Option([], type=valid_options),
                "pre_start_cmd_append": Option([], type=valid_options),

                "post_start_cmd": Option([
                    "/usr/bin/systemd-run",
                    "--unit=kvmd-otgnet-dnsmasq",
                    "/usr/sbin/dnsmasq",
                    "--conf-file=/dev/null",
                    "--pid-file",
                    "--user=dnsmasq",
                    "--interface={iface}",
                    "--port=0",
                    "--dhcp-range={dhcp_ip_begin},{dhcp_ip_end},24h",
                    "--dhcp-leasefile=/run/kvmd/dnsmasq.lease",
                    "--dhcp-option={dhcp_option_3}",
                    "--dhcp-option=6",
                    "--keep-in-foreground",
                ], type=valid_command),
                "post_start_cmd_remove": Option([], type=valid_options),
                "post_start_cmd_append": Option([], type=valid_options),

                "pre_stop_cmd": Option([
                    "/usr/bin/systemctl",
                    "stop",
                    "kvmd-otgnet-dnsmasq",
                ], type=valid_command),
                "pre_stop_cmd_remove": Option([], type=valid_options),
                "pre_stop_cmd_append": Option([], type=valid_options),

                "post_stop_cmd":        Option(["/bin/true", "post-stop"], type=valid_command),
                "post_stop_cmd_remove": Option([], type=valid_options),
                "post_stop_cmd_append": Option([], type=valid_options),
            },
        },

        "ipmi": {
            "server": {
                "host":    Option("",   type=valid_ip_or_host, if_empty=""),
                "port":    Option(623,  type=valid_port),
                "timeout": Option(10.0, type=valid_float_f01),
            },

            "kvmd": {
                "unix":    Option("/run/kvmd/kvmd.sock", type=valid_abs_path, unpack_as="unix_path"),
                "timeout": Option(5.0, type=valid_float_f01),
            },

            "auth": {
                "file": Option("/etc/kvmd/ipmipasswd", type=valid_abs_file, unpack_as="path"),
            },

            "sol": {
                "device":         Option("",     type=valid_abs_path, if_empty="", unpack_as="sol_device_path"),
                "speed":          Option(115200, type=valid_tty_speed, unpack_as="sol_speed"),
                "select_timeout": Option(0.1,    type=valid_float_f01, unpack_as="sol_select_timeout"),
                "proxy_port":     Option(0,      type=valid_port, unpack_as="sol_proxy_port"),
            },
        },

        "vnc": {
            "desired_fps":     Option(30, type=valid_stream_fps),
            "mouse_output":    Option("usb", type=valid_hid_mouse_output),
            "keymap":          Option("/usr/share/kvmd/keymaps/en-us", type=valid_abs_file),
            "allow_cut_after": Option(3.0, type=valid_float_f0),

            "server": {
                "host":        Option("",   type=valid_ip_or_host, if_empty=""),
                "port":        Option(5900, type=valid_port),
                "max_clients": Option(10,   type=valid_int_f1),

                "no_delay": Option(True, type=valid_bool),
                "keepalive": {
                    "enabled":  Option(True, type=valid_bool, unpack_as="keepalive_enabled"),
                    "idle":     Option(10,   type=functools.partial(valid_number, min=1, max=3600), unpack_as="keepalive_idle"),
                    "interval": Option(3,    type=functools.partial(valid_number, min=1, max=60), unpack_as="keepalive_interval"),
                    "count":    Option(3,    type=functools.partial(valid_number, min=1, max=10), unpack_as="keepalive_count"),
                },

                "tls": {
                    "ciphers": Option("ALL:@SECLEVEL=0", type=valid_ssl_ciphers, if_empty=""),
                    "timeout": Option(30.0, type=valid_float_f01),
                    "x509": {
                        "cert": Option("/etc/kvmd/vnc/ssl/server.crt", type=valid_abs_file, if_empty=""),
                        "key":  Option("/etc/kvmd/vnc/ssl/server.key", type=valid_abs_file, if_empty=""),
                    },
                },
            },

            "kvmd": {
                "unix":    Option("/run/kvmd/kvmd.sock", type=valid_abs_path, unpack_as="unix_path"),
                "timeout": Option(5.0, type=valid_float_f01),
            },

            "streamer": {
                "unix":    Option("/run/kvmd/ustreamer.sock", type=valid_abs_path, unpack_as="unix_path"),
                "timeout": Option(5.0, type=valid_float_f01),
            },

            "memsink": {
                "jpeg": {
                    "sink":             Option("",  unpack_as="obj"),
                    "lock_timeout":     Option(1.0, type=valid_float_f01),
                    "wait_timeout":     Option(1.0, type=valid_float_f01),
                    "drop_same_frames": Option(1.0, type=valid_float_f0),
                },
                "h264": {
                    "sink":             Option("",  unpack_as="obj"),
                    "lock_timeout":     Option(1.0, type=valid_float_f01),
                    "wait_timeout":     Option(1.0, type=valid_float_f01),
                    "drop_same_frames": Option(0.0, type=valid_float_f0),
                },
            },

            "auth": {
                "vncauth": {
                    "enabled": Option(False, type=valid_bool),
                    "file":    Option("/etc/kvmd/vncpasswd", type=valid_abs_file, unpack_as="path"),
                },
                "vencrypt": {
                    "enabled": Option(True, type=valid_bool, unpack_as="vencrypt_enabled"),
                },
            },
        },

        "nginx": {
            "http": {
                "port": Option(80, type=valid_port),
            },
            "https": {
                "enabled": Option(True, type=valid_bool),
                "port":    Option(443,  type=valid_port),
            },
        },

        "janus": {
            "stun": {
                "host":          Option("stun.l.google.com", type=valid_ip_or_host, unpack_as="stun_host"),
                "port":          Option(19302, type=valid_port, unpack_as="stun_port"),
                "timeout":       Option(5.0,   type=valid_float_f01, unpack_as="stun_timeout"),
                "retries":       Option(5,     type=valid_int_f1, unpack_as="stun_retries"),
                "retries_delay": Option(5.0,   type=valid_float_f01, unpack_as="stun_retries_delay"),
            },

            "check": {
                "interval":      Option(10.0, type=valid_float_f01, unpack_as="check_interval"),
                "retries":       Option(5,    type=valid_int_f1, unpack_as="check_retries"),
                "retries_delay": Option(5.0,  type=valid_float_f01, unpack_as="check_retries_delay"),
            },

            "cmd": Option([
                "/usr/bin/janus",
                "--disable-colors",
                "--plugins-folder=/usr/lib/ustreamer/janus",
                "--configs-folder=/etc/kvmd/janus",
                "--interface={src_ip}",
                "{o_stun_server}",
            ], type=valid_command),
            "cmd_remove": Option([], type=valid_options),
            "cmd_append": Option([], type=valid_options),
        },

        "watchdog": {
            "rtc":      Option(0,   type=valid_int_f0),
            "timeout":  Option(300, type=valid_int_f1),
            "interval": Option(30,  type=valid_int_f1),
        },
    }
