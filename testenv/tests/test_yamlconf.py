# ========================================================================== #
#                                                                            #
#    KVMD - The main PiKVM daemon.                                           #
#                                                                            #
#    Copyright (C) 2018-2021  Maxim Devaev <mdevaev@gmail.com>               #
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


import pathlib
import textwrap

from kvmd.yamlconf.loader import load_yaml_file


# =====
def test_load_yaml_file__bools(tmp_path: pathlib.Path) -> None:  # type: ignore
    pobj = tmp_path / "test.yaml"
    pobj.write_text(textwrap.dedent("""
        a: true
        b: false
        c: yes
        d: no
    """))
    data = load_yaml_file(str(pobj))
    assert data["a"] is True
    assert data["b"] is False
    assert data["c"] == "yes"
    assert data["d"] == "no"
