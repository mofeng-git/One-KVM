import json

from typing import Tuple
from typing import List
from typing import Dict
from typing import Callable
from typing import Optional
from typing import Any


# =====
def build_raw_from_options(options: List[str]) -> Dict[str, Any]:
    raw: Dict[str, Any] = {}
    for option in options:
        (key, value) = (option.split("=", 1) + [None])[:2]  # type: ignore
        if len(key.strip()) == 0:
            raise ValueError("Empty option key (required 'key=value' instead of '{}')".format(option))
        if value is None:
            raise ValueError("No value for key '{}'".format(key))

        section = raw
        subs = list(map(str.strip, key.split("/")))
        for sub in subs[:-1]:
            section.setdefault(sub, {})
            section = section[sub]
        section[subs[-1]] = _parse_value(value)
    return raw


def _parse_value(value: str) -> Any:
    value = value.strip()
    if (
        not value.isdigit()
        and value not in ["true", "false", "null"]
        and not value.startswith(("{", "[", "\""))
    ):
        value = "\"{}\"".format(value)
    return json.loads(value)


# =====
class Section(dict):
    def __init__(self) -> None:
        dict.__init__(self)
        self.__meta: Dict[str, Dict[str, Any]] = {}

    def _set_meta(self, name: str, default: Any, help: str) -> None:  # pylint: disable=redefined-builtin
        self.__meta[name] = {
            "default": default,
            "help":    help,
        }

    def _get_default(self, name: str) -> Any:
        return self.__meta[name]["default"]

    def _get_help(self, name: str) -> str:
        return self.__meta[name]["help"]

    def __getattribute__(self, name: str) -> Any:
        if name in self:
            return self[name]
        else:  # For pickling
            return dict.__getattribute__(self, name)


class Option:
    __type = type

    def __init__(self, default: Any, help: str="", type: Optional[Callable[[Any], Any]]=None) -> None:  # pylint: disable=redefined-builtin
        self.default = default
        self.help = help
        self.type: Callable[[Any], Any] = (type or (self.__type(default) if default is not None else str))  # type: ignore

    def __repr__(self) -> str:
        return "<Option(default={self.default}, type={self.type}, help={self.help})>".format(self=self)


# =====
def make_config(raw: Dict[str, Any], scheme: Dict[str, Any], _keys: Tuple[str, ...]=()) -> Section:
    if not isinstance(raw, dict):
        raise ValueError("The node '{}' must be a dictionary".format("/".join(_keys) or "/"))

    config = Section()
    for (key, option) in scheme.items():
        full_key = _keys + (key,)
        full_name = "/".join(full_key)

        if isinstance(option, Option):
            value = raw.get(key, option.default)
            try:
                value = option.type(value)
            except Exception:
                raise ValueError("Invalid value '{value}' for key '{key}'".format(key=full_name, value=value))
            config[key] = value
            config._set_meta(  # pylint: disable=protected-access
                name=key,
                default=option.default,
                help=option.help,
            )
        elif isinstance(option, dict):
            config[key] = make_config(raw.get(key, {}), option, full_key)
        else:
            raise RuntimeError("Incorrect scheme definition for key '{}':"
                               " the value is {}, not dict or Option()".format(full_name, type(option)))
    return config
