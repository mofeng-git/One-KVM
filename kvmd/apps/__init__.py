import sys
import os
import argparse
import logging
import logging.config

from typing import Tuple
from typing import List
from typing import Dict
from typing import Sequence
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
def init(prog: str=sys.argv[0], add_help: bool=True) -> Tuple[argparse.ArgumentParser, List[str], Section]:
    args_parser = argparse.ArgumentParser(prog=prog, add_help=add_help)
    args_parser.add_argument("-c", "--config", dest="config_path", default="/etc/kvmd/kvmd.yaml", metavar="<file>")
    args_parser.add_argument("-o", "--set-options", dest="set_options", default=[], nargs="+")
    args_parser.add_argument("-m", "--dump-config", dest="dump_config", action="store_true")
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
    if not isinstance(pin, int) or pin == 0:
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
                "host": Option(default="localhost"),
                "port": Option(default=0),
                "unix": Option(default="", type=_as_optional_path),
                "unix_rm": Option(default=False),
                "unix_mode": Option(default=0),
                "heartbeat": Option(default=3.0),
                "access_log_format": Option(default="[%P / %{X-Real-IP}i] '%r' => %s; size=%b ---"
                                                    " referer='%{Referer}i'; user_agent='%{User-Agent}i'"),
            },

            "auth": {
                "htpasswd": Option(default="/etc/kvmd/htpasswd", type=_as_path),
            },

            "info": {
                "meta":   Option(default="/etc/kvmd/meta.yaml", type=_as_path),
                "extras": Option(default="/usr/share/kvmd/extras", type=_as_path),
            },

            "hid": {
                "pinout": {
                    "reset": Option(default=0, type=_as_pin),
                },
                "reset_delay":    Option(default=0.1),
                "device":         Option(default="", type=_as_path),
                "speed":          Option(default=115200),
                "read_timeout":   Option(default=2.0),
                "read_retries":   Option(default=10),
                "common_retries": Option(default=100),
                "retries_delay":  Option(default=0.1),
                "noop":           Option(default=False),
                "state_poll":     Option(default=0.1),
            },

            "atx": {
                "pinout": {
                    "power_led":    Option(default=0, type=_as_pin),
                    "hdd_led":      Option(default=0, type=_as_pin),
                    "power_switch": Option(default=0, type=_as_pin),
                    "reset_switch": Option(default=0, type=_as_pin),
                },
                "click_delay":      Option(default=0.1),
                "long_click_delay": Option(default=5.5),
                "state_poll":       Option(default=0.1),
            },

            "msd": {
                "pinout": {
                    "target": Option(default=0, type=_as_pin),
                    "reset":  Option(default=0, type=_as_pin),
                },
                "device":      Option(default="", type=_as_path),
                "init_delay":  Option(default=2.0),
                "reset_delay": Option(default=1.0),
                "write_meta":  Option(default=True),
                "chunk_size":  Option(default=65536),
            },

            "streamer": {
                "pinout": {
                    "cap":  Option(default=-1, type=_as_optional_pin),
                    "conv": Option(default=-1, type=_as_optional_pin),
                },

                "sync_delay":         Option(default=1.0),
                "init_delay":         Option(default=1.0),
                "init_restart_after": Option(default=0.0),
                "shutdown_delay":     Option(default=10.0),
                "state_poll":         Option(default=1.0),

                "quality":     Option(default=80),
                "desired_fps": Option(default=0),

                "host":    Option(default="localhost"),
                "port":    Option(default=0),
                "unix":    Option(default="", type=_as_optional_path),
                "timeout": Option(default=2.0),

                "cmd": Option(default=["/bin/true"], type=_as_string_list),
            },
        },

        "logging": Option(default={}),
    }
