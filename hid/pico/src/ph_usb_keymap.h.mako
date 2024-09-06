/*****************************************************************************
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
*****************************************************************************/


#pragma once

#include "ph_types.h"

<%! import operator %>
inline u8 ph_usb_keymap(u8 key) {
	switch (key) {
% for km in sorted(keymap, key=operator.attrgetter("mcu_code")):
	% if km.usb_key.is_modifier:
		case ${km.mcu_code}: return ${km.usb_key.arduino_modifier_code}; // ${km.web_name}
	% else:
		case ${km.mcu_code}: return ${km.usb_key.code}; // ${km.web_name}
	% endif
% endfor
	}
	return 0;
}
