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


import pytest

from kvmd.yamlconf import merger


# =====
def test_simple_override() -> None:
    base = {"key1": "value1", "key2": "value2"}
    incoming = {"key1": "new_value1"}
    merger.yaml_merge(base, incoming)
    assert base == {"key1": "new_value1", "key2": "value2"}


def test_nested_override() -> None:
    base = {"key1": {"nested_key1": "value1"}, "key2": "value2"}
    incoming = {"key1": {"nested_key1": "new_value1"}}
    merger.yaml_merge(base, incoming)
    assert base == {"key1": {"nested_key1": "new_value1"}, "key2": "value2"}


def test_dest_none() -> None:
    base = None
    incoming = {"key1": "value1"}
    with pytest.raises(ValueError, match="destination cannot be None"):
        merger.yaml_merge(base, incoming)  # type: ignore[arg-type]


def test_src_none_or_empty() -> None:
    base = {"key1": "value1"}
    incoming = None
    merger.yaml_merge(base, incoming)  # type: ignore[arg-type]
    assert base == {"key1": "value1"}

    base = {"key1": "value1"}
    incoming2: dict = {}
    merger.yaml_merge(base, incoming2)
    assert base == {"key1": "value1"}


def test_merged_new_keys() -> None:
    base = {"key1": "value1"}
    incoming = {"key2": "value2"}
    merger.yaml_merge(base, incoming)
    assert base == {"key1": "value1", "key2": "value2"}


def test_dest_not_dict() -> None:
    base = "I'm not a dict"
    incoming = {"key1": "value1"}
    with pytest.raises(TypeError, match="object does not support item assignment"):
        merger.yaml_merge(base, incoming)  # type: ignore[arg-type]


def test_src_not_dict() -> None:
    base = {"key1": "value1"}
    incoming = "I'm not a dict"
    with pytest.raises(TypeError, match="string indices must be integers, not 'str'"):
        merger.yaml_merge(base, incoming)  # type: ignore[arg-type]


def test_nested_lists_overwrite() -> None:
    base = {"key1": [1, 2, 3]}
    incoming = {"key1": ["a", "b", "c"]}
    merger.yaml_merge(base, incoming)
    assert base == {"key1": ["a", "b", "c"]}


def test_same_information_rewrite() -> None:
    base = {"key1": "value1", "key2": "value2"}
    incoming = {"key1": "value1", "key2": "value2"}
    merger.yaml_merge(base, incoming)
    assert base == {"key1": "value1", "key2": "value2"}


def test_deeply_nested_dictionaries() -> None:
    base = {"key1": {"nested_key1": {"deep_nested_key1": "value1"}}, "key2": "value2"}
    incoming = {"key1": {"nested_key1": {"deep_nested_key1": "new_value1"}}}
    merger.yaml_merge(base, incoming)
    assert base == {"key1": {"nested_key1": {"deep_nested_key1": "new_value1"}}, "key2": "value2"}


def test_non_dict_values_in_source() -> None:
    base = {"key1": "value1", "key2": "value2"}
    incoming = {"key1": 123, "key2": ["value3", "value4"]}
    merger.yaml_merge(base, incoming)
    assert base == {"key1": 123, "key2": ["value3", "value4"]}


def test_empty_base() -> None:
    base: dict = {}
    incoming = {"key1": "value1"}
    merger.yaml_merge(base, incoming)
    assert base == {"key1": "value1"}


def test_none_values_in_source() -> None:
    base = {"key1": "value1", "key2": "value2"}
    incoming = {"key1": None, "key2": "new_value2"}
    merger.yaml_merge(base, incoming)
    assert base == {"key1": None, "key2": "new_value2"}


def test_key_not_present_in_incoming() -> None:
    base = {"key1": "value1", "key2": "value2"}
    incoming = {"key3": "value3"}
    merger.yaml_merge(base, incoming)
    assert base == {"key1": "value1", "key2": "value2", "key3": "value3"}


def test_mixed_nested_non_nested_keys() -> None:
    base = {"key1": "value1", "key2": {"nested_key1": "value2"}}
    incoming = {"key1": "new_value1", "key2": {"nested_key1": "new_value2"}}
    merger.yaml_merge(base, incoming)
    assert base == {"key1": "new_value1", "key2": {"nested_key1": "new_value2"}}


def test_additional_nested_keys_in_incoming() -> None:
    base = {"key1": "value1", "key2": {"nested_key1": "value2"}}
    incoming = {"key1": "new_value1", "key2": {"nested_key1": "new_value2", "nested_key2": "value3"}}
    merger.yaml_merge(base, incoming)
    assert base == {"key1": "new_value1", "key2": {"nested_key1": "new_value2", "nested_key2": "value3"}}


def test_override_nested_dict_with_non_dict() -> None:
    base = {"key1": "value1", "key2": {"nested_key1": "value2"}}
    incoming = {"key1": "new_value1", "key2": "new_value2"}
    merger.yaml_merge(base, incoming)
    assert base == {"key1": "new_value1", "key2": "new_value2"}


def test_multiple_value_types() -> None:
    base = {"key1": 1, "key2": True, "key3": [1, 2, 3], "key4": {"nested_key1": "value1"}}
    incoming = {"key1": 2, "key2": False, "key3": [4, 5, 6], "key4": {"nested_key1": "value2"}}
    merger.yaml_merge(base, incoming)
    assert base == {"key1": 2, "key2": False, "key3": [4, 5, 6], "key4": {"nested_key1": "value2"}}


def test_non_string_keys() -> None:
    base: dict = {1: "value1", 2: "value2"}
    incoming: dict = {1: "new_value1", 3: "value3"}
    merger.yaml_merge(base, incoming)
    assert base == {1: "new_value1", 2: "value2", 3: "value3"}
