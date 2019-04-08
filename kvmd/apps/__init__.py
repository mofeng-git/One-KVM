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

from ..yamlconf import ConfigError
from ..yamlconf import make_config
from ..yamlconf import Section
from ..yamlconf import Option
from ..yamlconf import build_raw_from_options
from ..yamlconf.dumper import make_config_dump
from ..yamlconf.loader import load_yaml_file

from ..validators.basic import valid_bool
from ..validators.basic import valid_number
from ..validators.basic import valid_int_f1
from ..validators.basic import valid_float_f01

from ..validators.fs import valid_abs_path
from ..validators.fs import valid_abs_path_exists
from ..validators.fs import valid_unix_mode

from ..validators.net import valid_ip_or_host
from ..validators.net import valid_port

from ..validators.auth import valid_auth_type

from ..validators.kvm import valid_stream_quality
from ..validators.kvm import valid_stream_fps

from ..validators.hw import valid_tty_speed
from ..validators.hw import valid_gpio_pin
from ..validators.hw import valid_gpio_pin_optional


# =====
def init(
    prog: Optional[str]=None,
    description: Optional[str]=None,
    add_help: bool=True,
    argv: Optional[List[str]]=None,
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
    raw_config: Dict = {}

    if options.config_path:
        options.config_path = os.path.expanduser(options.config_path)
        raw_config = load_yaml_file(options.config_path)

    scheme = _get_config_scheme()
    try:
        _merge_dicts(raw_config, build_raw_from_options(options.set_options))
        config = make_config(raw_config, scheme)
    except ConfigError as err:
        raise SystemExit("Config error: " + str(err))

    if options.dump_config:
        dump = make_config_dump(config)
        if sys.stdout.isatty():
            dump = pygments.highlight(
                dump,
                pygments.lexers.data.YamlLexer(),
                pygments.formatters.TerminalFormatter(bg="dark"),  # pylint: disable=no-member
            )
        print(dump)
        sys.exit(0)

    logging.captureWarnings(True)
    logging.config.dictConfig(config.logging)
    return (args_parser, remaining, config)


# =====
def _merge_dicts(dest: Dict, src: Dict) -> None:
    for key in src:
        if key in dest:
            if isinstance(dest[key], dict) and isinstance(src[key], dict):
                _merge_dicts(dest[key], src[key])
                continue
        dest[key] = src[key]


def _get_config_scheme() -> Dict:
    return {
        "kvmd": {
            "server": {
                "host":              Option("localhost", type=valid_ip_or_host),
                "port":              Option(0,     type=valid_port),
                "unix":              Option("",    type=valid_abs_path, only_if="!port", unpack_as="unix_path"),
                "unix_rm":           Option(False, type=valid_bool),
                "unix_mode":         Option(0,     type=valid_unix_mode),
                "heartbeat":         Option(3.0,   type=valid_float_f01),
                "access_log_format": Option("[%P / %{X-Real-IP}i] '%r' => %s; size=%b ---"
                                            " referer='%{Referer}i'; user_agent='%{User-Agent}i'"),
            },

            "auth": {
                "type": Option("htpasswd", type=valid_auth_type, unpack_as="auth_type"),
                "htpasswd": {
                    "file": Option("/etc/kvmd/htpasswd", type=valid_abs_path_exists, unpack_as="path"),
                },
            },

            "info": {
                "meta":   Option("/etc/kvmd/meta.yaml",    type=valid_abs_path_exists, unpack_as="meta_path"),
                "extras": Option("/usr/share/kvmd/extras", type=valid_abs_path_exists, unpack_as="extras_path"),
            },

            "hid": {
                "reset_pin":   Option(-1,  type=valid_gpio_pin),
                "reset_delay": Option(0.1, type=valid_float_f01),

                "device":         Option("",     type=valid_abs_path, unpack_as="device_path"),
                "speed":          Option(115200, type=valid_tty_speed),
                "read_timeout":   Option(2.0,    type=valid_float_f01),
                "read_retries":   Option(10,     type=valid_int_f1),
                "common_retries": Option(100,    type=valid_int_f1),
                "retries_delay":  Option(0.1,    type=valid_float_f01),
                "noop":           Option(False,  type=valid_bool),

                "state_poll": Option(0.1, type=valid_float_f01),
            },

            "atx": {
                "enabled": Option(True, type=valid_bool),

                "power_led_pin":    Option(-1, type=valid_gpio_pin, only_if="enabled"),
                "hdd_led_pin":      Option(-1, type=valid_gpio_pin, only_if="enabled"),
                "power_switch_pin": Option(-1, type=valid_gpio_pin, only_if="enabled"),
                "reset_switch_pin": Option(-1, type=valid_gpio_pin, only_if="enabled"),

                "click_delay":      Option(0.1, type=valid_float_f01),
                "long_click_delay": Option(5.5, type=valid_float_f01),

                "state_poll": Option(0.1, type=valid_float_f01),
            },

            "msd": {
                "enabled": Option(True, type=valid_bool),

                "target_pin": Option(-1, type=valid_gpio_pin, only_if="enabled"),
                "reset_pin":  Option(-1, type=valid_gpio_pin, only_if="enabled"),

                "device":      Option("",    type=valid_abs_path, only_if="enabled", unpack_as="device_path"),
                "init_delay":  Option(2.0,   type=valid_float_f01),
                "reset_delay": Option(1.0,   type=valid_float_f01),
                "write_meta":  Option(True,  type=valid_bool),
                "chunk_size":  Option(65536, type=(lambda arg: valid_number(arg, min=1024))),
            },

            "streamer": {
                "cap_pin":  Option(-1, type=valid_gpio_pin_optional),
                "conv_pin": Option(-1, type=valid_gpio_pin_optional),

                "sync_delay":         Option(1.0,  type=valid_float_f01),
                "init_delay":         Option(1.0,  type=valid_float_f01),
                "init_restart_after": Option(0.0,  type=(lambda arg: valid_number(arg, min=0.0, type=float))),
                "shutdown_delay":     Option(10.0, type=valid_float_f01),
                "state_poll":         Option(1.0,  type=valid_float_f01),

                "quality":     Option(80, type=valid_stream_quality),
                "desired_fps": Option(0,  type=valid_stream_fps),

                "host":    Option("localhost", type=valid_ip_or_host),
                "port":    Option(0,   type=valid_port),
                "unix":    Option("",  type=valid_abs_path, only_if="!port", unpack_as="unix_path"),
                "timeout": Option(2.0, type=valid_float_f01),

                "cmd": Option(["/bin/true"]),  # TODO: Validator
            },
        },

        "logging": Option({}),
    }
