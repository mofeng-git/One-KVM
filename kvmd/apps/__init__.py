# ========================================================================== #
#                                                                            #
#    KVMD - The The main Pi-KVM daemon.                                      #
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
from typing import Sequence
from typing import Optional
from typing import Union

import pygments
import pygments.lexers.data
import pygments.formatters

from ..yamlconf import make_config
from ..yamlconf import Section
from ..yamlconf import Option
from ..yamlconf import build_raw_from_options
from ..yamlconf.dumper import make_config_dump
from ..yamlconf.loader import load_yaml_file


# =====
def init(
    prog: str=sys.argv[0],
    description: Optional[str]=None,
    add_help: bool=True,
) -> Tuple[argparse.ArgumentParser, List[str], Section]:

    args_parser = argparse.ArgumentParser(prog=prog, description=description, add_help=add_help)
    args_parser.add_argument("-c", "--config", dest="config_path", default="/etc/kvmd/main.yaml", metavar="<file>",
                             help="Set config file path")
    args_parser.add_argument("-o", "--set-options", dest="set_options", default=[], nargs="+",
                             help="Override config options list (like sec/sub/opt=value)")
    args_parser.add_argument("-m", "--dump-config", dest="dump_config", action="store_true",
                             help="View current configuration (include all overrides)")
    (options, remaining) = args_parser.parse_known_args(sys.argv)

    options.config_path = os.path.expanduser(options.config_path)
    if os.path.exists(options.config_path):
        raw_config = load_yaml_file(options.config_path)
    else:
        raw_config = {}
    _merge_dicts(raw_config, build_raw_from_options(options.set_options))
    scheme = _get_config_scheme()
    config = make_config(raw_config, scheme)

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


def _as_pin(pin: int) -> int:
    if not isinstance(pin, int) or pin <= 0:
        raise ValueError("Invalid pin number")
    return pin


def _as_optional_pin(pin: int) -> int:
    if not isinstance(pin, int) or pin < -1:
        raise ValueError("Invalid optional pin number")
    return pin


def _as_path(path: str) -> str:
    if not isinstance(path, str):
        raise ValueError("Invalid path")
    path = str(path).strip()
    if not path:
        raise ValueError("Invalid path")
    return path


def _as_optional_path(path: str) -> str:
    if not isinstance(path, str):
        raise ValueError("Invalid path")
    return str(path).strip()


def _as_string_list(values: Union[str, Sequence]) -> List[str]:
    if isinstance(values, str):
        values = [values]
    return list(map(str, values))


def _get_config_scheme() -> Dict:
    return {
        "kvmd": {
            "server": {
                "host":              Option("localhost"),
                "port":              Option(0),
                "unix":              Option("", type=_as_optional_path, rename="unix_path"),
                "unix_rm":           Option(False),
                "unix_mode":         Option(0),
                "heartbeat":         Option(3.0),
                "access_log_format": Option("[%P / %{X-Real-IP}i] '%r' => %s; size=%b ---"
                                            " referer='%{Referer}i'; user_agent='%{User-Agent}i'"),
            },

            "auth": {
                "htpasswd": Option("/etc/kvmd/htpasswd", type=_as_path, rename="htpasswd_path"),
            },

            "info": {
                "meta":   Option("/etc/kvmd/meta.yaml", type=_as_path, rename="meta_path"),
                "extras": Option("/usr/share/kvmd/extras", type=_as_path, rename="extras_path"),
            },

            "hid": {
                "reset_pin":   Option(0, type=_as_pin),
                "reset_delay": Option(0.1),

                "device":         Option("", type=_as_path, rename="device_path"),
                "speed":          Option(115200),
                "read_timeout":   Option(2.0),
                "read_retries":   Option(10),
                "common_retries": Option(100),
                "retries_delay":  Option(0.1),
                "noop":           Option(False),

                "state_poll": Option(0.1),
            },

            "atx": {
                "enabled": Option(True),

                "power_led_pin": Option(-1, type=_as_optional_pin),
                "hdd_led_pin":   Option(-1, type=_as_optional_pin),
                "power_switch_pin": Option(-1, type=_as_optional_pin),
                "reset_switch_pin": Option(-1, type=_as_optional_pin),

                "click_delay":      Option(0.1),
                "long_click_delay": Option(5.5),

                "state_poll": Option(0.1),
            },

            "msd": {
                "enabled": Option(True),

                "target_pin": Option(-1, type=_as_optional_pin),
                "reset_pin":  Option(-1, type=_as_optional_pin),

                "device":      Option("", type=_as_optional_path, rename="device_path"),
                "init_delay":  Option(2.0),
                "reset_delay": Option(1.0),
                "write_meta":  Option(True),
                "chunk_size":  Option(65536),
            },

            "streamer": {
                "cap_pin":  Option(0, type=_as_optional_pin),
                "conv_pin": Option(0, type=_as_optional_pin),

                "sync_delay":         Option(1.0),
                "init_delay":         Option(1.0),
                "init_restart_after": Option(0.0),
                "shutdown_delay":     Option(10.0),
                "state_poll":         Option(1.0),

                "quality":     Option(80),
                "desired_fps": Option(0),

                "host":    Option("localhost"),
                "port":    Option(0),
                "unix":    Option("", type=_as_optional_path, rename="unix_path"),
                "timeout": Option(2.0),

                "cmd": Option(["/bin/true"], type=_as_string_list),
            },
        },

        "logging": Option({}),
    }
