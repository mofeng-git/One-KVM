#!/bin/bash
# ========================================================================== #
#                                                                            #
#    KVMD - The main PiKVM daemon.                                           #
#                                                                            #
#    Copyright (C) 2023-2025  SilentWind <mofeng654321@hotmail.com>         #
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
case $1 in
    short)
    python3 /etc/kvmd/custom_atx/usbrelay_hid.py 1 on
    sleep 1
    python3 /etc/kvmd/custom_atx/usbrelay_hid.py 1 off
    ;;
    long)
    python3 /etc/kvmd/custom_atx/usbrelay_hid.py 1 on
    sleep 5
    python3 /etc/kvmd/custom_atx/usbrelay_hid.py 1 off
    ;;
    reset)
    python3 /etc/kvmd/custom_atx/usbrelay_hid.py 2 on
    sleep 1
    python3 /etc/kvmd/custom_atx/usbrelay_hid.py 2 off
    ;;
    *)
    echo "No thing."
esac