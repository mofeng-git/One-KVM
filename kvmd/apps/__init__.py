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
from typing import Optional

import pygments
import pygments.lexers.data
import pygments.formatters

from ..plugins import UnknownPluginError
from ..plugins.auth import get_auth_service_class
from ..plugins.hid import get_hid_class
from ..plugins.atx import get_atx_class
from ..plugins.msd import get_msd_class

from ..yamlconf import ConfigError
from ..yamlconf import make_config
from ..yamlconf import Section
from ..yamlconf import Option
from ..yamlconf import build_raw_from_options
from ..yamlconf.dumper import make_config_dump
from ..yamlconf.loader import load_yaml_file

from ..validators.basic import valid_bool
from ..validators.basic import valid_number
from ..validators.basic import valid_float_f0
from ..validators.basic import valid_float_f01

from ..validators.auth import valid_users_list

from ..validators.os import valid_abs_path
from ..validators.os import valid_abs_path_exists
from ..validators.os import valid_unix_mode
from ..validators.os import valid_command

from ..validators.net import valid_ip_or_host
from ..validators.net import valid_port

from ..validators.kvm import valid_stream_quality
from ..validators.kvm import valid_stream_fps

from ..validators.hw import valid_gpio_pin_optional


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
                             type=valid_abs_path_exists, help="Set config file path")
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
        ))
        raise SystemExit()
    config = _init_config(options.config_path, options.set_options, **load)

    logging.captureWarnings(True)
    logging.config.dictConfig(config.logging)
    return (args_parser, remaining, config)


# =====
def _init_config(
    config_path: str,
    override_options: List[str],
    load_auth: bool=False,
    load_hid: bool=False,
    load_atx: bool=False,
    load_msd: bool=False,
) -> Section:

    config_path = os.path.expanduser(config_path)
    raw_config: Dict = load_yaml_file(config_path)

    scheme = _get_config_scheme()
    try:
        _merge_dicts(raw_config, build_raw_from_options(override_options))
        _merge_dicts(raw_config, (raw_config.pop("override", {}) or {}))
        config = make_config(raw_config, scheme)

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

        if rebuild:
            config = make_config(raw_config, scheme)

        return config
    except (ConfigError, UnknownPluginError) as err:
        raise SystemExit(f"Config error: {err}")


def _dump_config(config: Section) -> None:
    dump = make_config_dump(config)
    if sys.stdout.isatty():
        dump = pygments.highlight(
            dump,
            pygments.lexers.data.YamlLexer(),
            pygments.formatters.TerminalFormatter(bg="dark"),  # pylint: disable=no-member
        )
    print(dump)


def _merge_dicts(dest: Dict, src: Dict) -> None:
    for key in src:
        if key in dest:
            if isinstance(dest[key], dict) and isinstance(src[key], dict):
                _merge_dicts(dest[key], src[key])
                continue
        dest[key] = src[key]


def _get_config_scheme() -> Dict:
    return {
        "logging": Option({}),

        "kvmd": {
            "server": {
                "host":              Option("localhost", type=valid_ip_or_host),
                "port":              Option(0,     type=valid_port),
                "unix":              Option("",    type=valid_abs_path, only_if="!port", unpack_as="unix_path"),
                "unix_rm":           Option(False, type=valid_bool),
                "unix_mode":         Option(0,     type=valid_unix_mode),
                "heartbeat":         Option(3.0,   type=valid_float_f01),
                "sync_chunk_size":   Option(65536, type=(lambda arg: valid_number(arg, min=1024))),
                "access_log_format": Option("[%P / %{X-Real-IP}i] '%r' => %s; size=%b ---"
                                            " referer='%{Referer}i'; user_agent='%{User-Agent}i'"),
            },

            "auth": {
                "internal": {
                    "type":  Option("htpasswd"),
                    "force_users": Option([], type=valid_users_list),
                    # Dynamic content
                },

                "external": {
                    "type": Option(""),
                },
            },

            "info": {
                "meta":   Option("/etc/kvmd/meta.yaml",    type=valid_abs_path_exists, unpack_as="meta_path"),
                "extras": Option("/usr/share/kvmd/extras", type=valid_abs_path_exists, unpack_as="extras_path"),
            },

            "hid": {
                "type": Option("serial"),
                # Dynamic content
            },

            "atx": {
                "type": Option("gpio"),
                # Dynamic content
            },

            "msd": {
                "type": Option("relay"),
                # Dynamic content
            },

            "streamer": {
                "cap_pin":  Option(-1, type=valid_gpio_pin_optional),
                "conv_pin": Option(-1, type=valid_gpio_pin_optional),

                "sync_delay":         Option(1.0,  type=valid_float_f0),
                "init_delay":         Option(3.0,  type=valid_float_f0),
                "init_restart_after": Option(0.0,  type=valid_float_f0),
                "shutdown_delay":     Option(10.0, type=valid_float_f01),
                "state_poll":         Option(1.0,  type=valid_float_f01),

                "quality":     Option(80,  type=valid_stream_quality),
                "desired_fps": Option(0,   type=valid_stream_fps),
                "max_fps":     Option(120, type=valid_stream_fps),

                "host":    Option("localhost", type=valid_ip_or_host),
                "port":    Option(0,   type=valid_port),
                "unix":    Option("",  type=valid_abs_path, only_if="!port", unpack_as="unix_path"),
                "timeout": Option(2.0, type=valid_float_f01),

                "cmd": Option(["/bin/true"], type=valid_command),
            },
        },

        "otg": {
            "gadget":        Option("kvmd"),
            "vendor_id":     Option(0x1D6B, type=valid_number),  # Linux Foundation
            "product_id":    Option(0x0104, type=valid_number),  # Multifunction Composite Gadget
            "manufacturer":  Option("Pi-KVM"),
            "product":       Option("Composite KVM Device"),
            "serial_number": Option("CAFEBABE"),
            "udc":           Option(""),
            "init_delay":    Option(3.0, type=valid_float_f01),

            "acm": {
                "enabled": Option(True, type=valid_bool),
            },
        },

        "ipmi": {
            "server": {
                "host":    Option("::", type=valid_ip_or_host),
                "port":    Option(623,  type=valid_port),
                "timeout": Option(10.0, type=valid_float_f01),
            },

            "kvmd": {
                "host":    Option("localhost", type=valid_ip_or_host, unpack_as="kvmd_host"),
                "port":    Option(0,   type=valid_port, unpack_as="kvmd_port"),
                "unix":    Option("",  type=valid_abs_path, only_if="!port", unpack_as="kvmd_unix_path"),
                "timeout": Option(5.0, type=valid_float_f01, unpack_as="kvmd_timeout"),
            },

            "auth": {
                "file": Option("/etc/kvmd/ipmipasswd", type=valid_abs_path_exists, unpack_as="path"),
            },
        },
    }
