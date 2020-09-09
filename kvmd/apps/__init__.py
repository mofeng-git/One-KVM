# ========================================================================== #
#                                                                            #
#    KVMD - The main Pi-KVM daemon.                                          #
#                                                                            #
#    Copyright (C) 2018  Maxim Devaev <mdevaev@gmail.com>                    #
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
import argparse
import logging
import logging.config

from typing import Tuple
from typing import List
from typing import Dict
from typing import Set
from typing import Optional

import pygments
import pygments.lexers.data
import pygments.formatters

from .. import tools

from ..plugins import UnknownPluginError
from ..plugins.auth import get_auth_service_class
from ..plugins.hid import get_hid_class
from ..plugins.atx import get_atx_class
from ..plugins.msd import get_msd_class
from ..plugins.ugpio import get_ugpio_driver_class

from ..yamlconf import ConfigError
from ..yamlconf import manual_validated
from ..yamlconf import make_config
from ..yamlconf import Section
from ..yamlconf import Option
from ..yamlconf import build_raw_from_options
from ..yamlconf.dumper import make_config_dump
from ..yamlconf.loader import load_yaml_file

from ..validators.basic import valid_stripped_string
from ..validators.basic import valid_stripped_string_not_empty
from ..validators.basic import valid_bool
from ..validators.basic import valid_number
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
from ..validators.os import valid_command

from ..validators.net import valid_ip_or_host
from ..validators.net import valid_ip
from ..validators.net import valid_port
from ..validators.net import valid_mac
from ..validators.net import valid_ssl_ciphers

from ..validators.kvm import valid_stream_quality
from ..validators.kvm import valid_stream_fps
from ..validators.kvm import valid_stream_resolution
from ..validators.kvm import valid_hid_key
from ..validators.kvm import valid_hid_mouse_move
from ..validators.kvm import valid_ugpio_driver
from ..validators.kvm import valid_ugpio_channel
from ..validators.kvm import valid_ugpio_mode
from ..validators.kvm import valid_ugpio_view_table

from ..validators.hw import valid_gpio_pin
from ..validators.hw import valid_gpio_pin_optional
from ..validators.hw import valid_otg_gadget
from ..validators.hw import valid_otg_id


# =====
def init(
    prog: Optional[str]=None,
    description: Optional[str]=None,
    add_help: bool=True,
    argv: Optional[List[str]]=None,
    **load: bool,
) -> Tuple[argparse.ArgumentParser, List[str], Section]:

    argv = (argv or sys.argv)
    assert len(argv) > 0

    args_parser = argparse.ArgumentParser(prog=(prog or argv[0]), description=description, add_help=add_help)
    args_parser.add_argument("-c", "--config", dest="config_path", default="/etc/kvmd/main.yaml", metavar="<file>",
                             type=valid_abs_file, help="Set config file path")
    args_parser.add_argument("-o", "--set-options", dest="set_options", default=[], nargs="+",
                             help="Override config options list (like sec/sub/opt=value)")
    args_parser.add_argument("-m", "--dump-config", dest="dump_config", action="store_true",
                             help="View current configuration (include all overrides)")
    (options, remaining) = args_parser.parse_known_args(argv)

    if options.dump_config:
        _dump_config(_init_config(
            config_path=options.config_path,
            override_options=options.set_options,
            load_auth=True,
            load_hid=True,
            load_atx=True,
            load_msd=True,
            load_gpio=True,
        ))
        raise SystemExit()
    config = _init_config(options.config_path, options.set_options, **load)

    logging.captureWarnings(True)
    logging.config.dictConfig(config.logging)
    return (args_parser, remaining, config)


# =====
def _init_config(config_path: str, override_options: List[str], **load_flags: bool) -> Section:
    config_path = os.path.expanduser(config_path)
    raw_config: Dict = load_yaml_file(config_path)

    scheme = _get_config_scheme()
    try:
        tools.merge(raw_config, (raw_config.pop("override", {}) or {}))
        tools.merge(raw_config, build_raw_from_options(override_options))
        config = make_config(raw_config, scheme)

        if _patch_dynamic(raw_config, config, scheme, **load_flags):
            config = make_config(raw_config, scheme)

        return config
    except (ConfigError, UnknownPluginError) as err:
        raise SystemExit(f"Config error: {err}")


def _patch_dynamic(  # pylint: disable=too-many-locals
    raw_config: Dict,
    config: Section,
    scheme: Dict,
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
        drivers: Set[str] = set()
        for (driver, params) in {  # type: ignore
            "__gpio__": {},
            **tools.rget(raw_config, "kvmd", "gpio", "drivers"),
        }.items():
            with manual_validated(driver, "kvmd", "gpio", "drivers", "<key>"):
                driver = valid_ugpio_driver(driver)
            driver_type = valid_stripped_string_not_empty(params.get("type", "gpio"))
            scheme["kvmd"]["gpio"]["drivers"][driver] = {
                "type": Option(driver_type, type=valid_stripped_string_not_empty),
                **get_ugpio_driver_class(driver_type).get_plugin_options()
            }
            drivers.add(driver)

        for (channel, params) in tools.rget(raw_config, "kvmd", "gpio", "scheme").items():
            with manual_validated(channel, "kvmd", "gpio", "scheme", "<key>"):
                channel = valid_ugpio_channel(channel)
            with manual_validated(params.get("mode", ""), "kvmd", "gpio", "scheme", channel, "mode"):
                mode = valid_ugpio_mode(params.get("mode", ""))
            scheme["kvmd"]["gpio"]["scheme"][channel] = {
                "driver":   Option("__gpio__", type=(lambda arg: valid_ugpio_driver(arg, drivers))),
                "pin":      Option(-1, type=valid_gpio_pin),
                "mode":     Option("", type=valid_ugpio_mode),
                "inverted": Option(False, type=valid_bool),
                **({
                    "busy_delay": Option(0.2, type=valid_float_f01),
                    "initial":    Option(False, type=(lambda arg: (None if arg is None else valid_bool(arg)))),
                    "switch":     Option(True, type=valid_bool),
                    "pulse": {  # type: ignore
                        "delay":     Option(0.1, type=valid_float_f0),
                        "min_delay": Option(0.1, type=valid_float_f01),
                        "max_delay": Option(0.1, type=valid_float_f01),
                    },
                } if mode == "output" else {})
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


def _get_config_scheme() -> Dict:
    return {
        "logging": Option({}),

        "kvmd": {
            "server": {
                "host":              Option("localhost", type=valid_ip_or_host),
                "port":              Option(0,     type=valid_port),
                "unix":              Option("",    type=valid_abs_path, only_if="!port", unpack_as="unix_path"),
                "unix_rm":           Option(True,  type=valid_bool),
                "unix_mode":         Option(0o660, type=valid_unix_mode),
                "heartbeat":         Option(3.0,   type=valid_float_f01),
                "sync_chunk_size":   Option(65536, type=(lambda arg: valid_number(arg, min=1024))),
                "access_log_format": Option("[%P / %{X-Real-IP}i] '%r' => %s; size=%b ---"
                                            " referer='%{Referer}i'; user_agent='%{User-Agent}i'"),
            },

            "auth": {
                "enabled": Option(True, type=valid_bool),

                "internal": {
                    "type":  Option("htpasswd"),
                    "force_users": Option([], type=valid_users_list),
                    # Dynamic content
                },

                "external": {
                    "type": Option("", type=valid_stripped_string),
                    # Dynamic content
                },
            },

            "info": {  # Accessed via global config, see kvmd/info for details
                "meta":   Option("/etc/kvmd/meta.yaml",    type=valid_abs_file),
                "extras": Option("/usr/share/kvmd/extras", type=valid_abs_dir),
                "hw": {
                    "vcgencmd_cmd":  Option(["/opt/vc/bin/vcgencmd"], type=valid_command),
                    "procfs_prefix": Option("", type=(lambda arg: str(arg).strip())),
                    "sysfs_prefix":  Option("", type=(lambda arg: str(arg).strip())),
                    "state_poll":    Option(10.0,  type=valid_float_f01),
                },
            },

            "wol": {
                "ip":   Option("255.255.255.255", type=(lambda arg: valid_ip(arg, v6=False))),
                "port": Option(9, type=valid_port),
                "mac":  Option("", type=(lambda arg: (valid_mac(arg) if arg else ""))),
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
                "cap_pin":  Option(-1, type=valid_gpio_pin_optional),
                "conv_pin": Option(-1, type=valid_gpio_pin_optional),

                "sync_delay":         Option(0.0,  type=valid_float_f0),
                "init_delay":         Option(1.0,  type=valid_float_f0),
                "init_restart_after": Option(0.0,  type=valid_float_f0),
                "shutdown_delay":     Option(10.0, type=valid_float_f01),
                "state_poll":         Option(1.0,  type=valid_float_f01),

                "quality":     Option(80, type=(lambda arg: (valid_stream_quality(arg) if arg else 0))),  # 0 for disabled feature
                "desired_fps": Option(30, type=valid_stream_fps),
                "max_fps":     Option(60, type=valid_stream_fps),
                "resolution":  Option("", type=(lambda arg: (valid_stream_resolution(arg) if arg else ""))),
                "available_resolutions": Option([], type=(lambda arg: valid_string_list(arg, subval=valid_stream_resolution))),

                "host":    Option("localhost", type=valid_ip_or_host),
                "port":    Option(0,   type=valid_port),
                "unix":    Option("",  type=valid_abs_path, only_if="!port", unpack_as="unix_path"),
                "timeout": Option(2.0, type=valid_float_f01),

                "process_name_prefix": Option("kvmd/streamer"),

                "cmd": Option(["/bin/true"], type=valid_command),
            },

            "snapshot": {
                "idle_interval": Option(0.0, type=valid_float_f0),
                "live_interval": Option(0.0, type=valid_float_f0),

                "wakeup_key":  Option("", type=(lambda arg: (valid_hid_key(arg) if arg else ""))),
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
                        "title": Option("GPIO"),
                    },
                    "table": Option([], type=valid_ugpio_view_table),
                },
            },
        },

        "otg": {
            "vendor_id":    Option(0x1D6B, type=valid_otg_id),  # Linux Foundation
            "product_id":   Option(0x0104, type=valid_otg_id),  # Multifunction Composite Gadget
            "manufacturer": Option("Pi-KVM"),
            "product":      Option("Composite KVM Device"),
            "serial":       Option("CAFEBABE"),

            "gadget":     Option("kvmd", type=valid_otg_gadget),
            "udc":        Option("",     type=valid_stripped_string),
            "init_delay": Option(3.0,    type=valid_float_f01),

            "msd": {
                "user": Option("kvmd", type=valid_user),
                "default": {
                    "stall":     Option(False, type=valid_bool),
                    "cdrom":     Option(True,  type=valid_bool),
                    "rw":        Option(False, type=valid_bool),
                    "removable": Option(True,  type=valid_bool),
                    "fua":       Option(True,  type=valid_bool),
                },
            },

            "acm": {
                "enabled": Option(False, type=valid_bool),
            },

            "drives": {
                "enabled": Option(False, type=valid_bool),
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

        "ipmi": {
            "server": {
                "host":    Option("::", type=valid_ip_or_host),
                "port":    Option(623,  type=valid_port),
                "timeout": Option(10.0, type=valid_float_f01),
            },

            "kvmd": {
                "host":    Option("localhost", type=valid_ip_or_host),
                "port":    Option(0,   type=valid_port),
                "unix":    Option("",  type=valid_abs_path, only_if="!port", unpack_as="unix_path"),
                "timeout": Option(5.0, type=valid_float_f01),
            },

            "auth": {
                "file": Option("/etc/kvmd/ipmipasswd", type=valid_abs_file, unpack_as="path"),
            },
        },

        "vnc": {
            "desired_fps": Option(30, type=valid_stream_fps),
            "keymap":      Option("/usr/share/kvmd/keymaps/en-us", type=valid_abs_file),

            "server": {
                "host":        Option("::", type=valid_ip_or_host),
                "port":        Option(5900, type=valid_port),
                "max_clients": Option(10,   type=valid_int_f1),

                "no_delay": Option(True, type=valid_bool),
                "keepalive": {
                    "enabled":  Option(True, type=valid_bool, unpack_as="keepalive_enabled"),
                    "idle":     Option(10, type=(lambda arg: valid_number(arg, min=1, max=3600)), unpack_as="keepalive_idle"),
                    "interval": Option(3, type=(lambda arg: valid_number(arg, min=1, max=60)), unpack_as="keepalive_interval"),
                    "count":    Option(3, type=(lambda arg: valid_number(arg, min=1, max=10)), unpack_as="keepalive_count"),
                },

                "tls": {
                    "ciphers": Option("ALL:@SECLEVEL=0", type=(lambda arg: valid_ssl_ciphers(arg) if arg else "")),
                    "timeout": Option(5.0, type=valid_float_f01),
                },
            },

            "kvmd": {
                "host":    Option("localhost", type=valid_ip_or_host),
                "port":    Option(0,   type=valid_port),
                "unix":    Option("",  type=valid_abs_path, only_if="!port", unpack_as="unix_path"),
                "timeout": Option(5.0, type=valid_float_f01),
            },

            "streamer": {
                "host":    Option("localhost", type=valid_ip_or_host),
                "port":    Option(0,   type=valid_port),
                "unix":    Option("",  type=valid_abs_path, only_if="!port", unpack_as="unix_path"),
                "timeout": Option(5.0, type=valid_float_f01),
            },

            "auth": {
                "vncauth": {
                    "enabled": Option(False, type=valid_bool),
                    "file":    Option("/etc/kvmd/vncpasswd", type=valid_abs_file, unpack_as="path"),
                },
            },
        },
    }
