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


import pytest

from kvmd import gpio


# =====
@pytest.mark.parametrize("pin", [0, 1, 13])
def test_ok__loopback_initial_false(pin: int) -> None:
    with gpio.bcm():
        assert gpio.set_output(pin) == pin
        assert gpio.read(pin) is False
        gpio.write(pin, True)
        assert gpio.read(pin) is True


@pytest.mark.parametrize("pin", [0, 1, 13])
def test_ok__loopback_initial_true(pin: int) -> None:
    with gpio.bcm():
        assert gpio.set_output(pin, True) == pin
        assert gpio.read(pin) is True
        gpio.write(pin, False)
        assert gpio.read(pin) is False


@pytest.mark.parametrize("pin", [0, 1, 13])
def test_ok__input(pin: int) -> None:
    with gpio.bcm():
        assert gpio.set_input(pin) == pin
        assert gpio.read(pin) is False


def test_fail__invalid_pin() -> None:
    with pytest.raises(AssertionError):
        gpio.set_output(-1)
    with pytest.raises(AssertionError):
        gpio.set_input(-1)
