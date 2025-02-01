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
    gpioset -m time -s 1 SHUTDOWNPIN=0
    gpioset SHUTDOWNPIN=1
    ;;
    long)
    gpioset -m time -s 5 SHUTDOWNPIN=0
    gpioset SHUTDOWNPIN=1
    ;;
    reset)
    gpioset -m time -s 1 REBOOTPIN=0
    gpioset REBOOTPIN=1
    ;;
    *)
    echo "No thing."
esac