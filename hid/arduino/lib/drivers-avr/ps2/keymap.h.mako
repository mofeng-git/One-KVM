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


enum Ps2KeyType : uint8_t {
	PS2_KEY_TYPE_UNKNOWN = 0,
	PS2_KEY_TYPE_REG = 1,
	PS2_KEY_TYPE_SPEC = 2,
	PS2_KEY_TYPE_PRINT = 3,
	PS2_KEY_TYPE_PAUSE = 4,
};

<%! import operator %>
void keymapPs2(uint8_t code, Ps2KeyType *ps2_type, uint8_t *ps2_code) {
	*ps2_type = PS2_KEY_TYPE_UNKNOWN;
	*ps2_code = 0;

	switch (code) {
% for km in sorted(keymap, key=operator.attrgetter("mcu_code")):
		case ${km.mcu_code}: *ps2_type = PS2_KEY_TYPE_${km.ps2_key.type.upper()}; *ps2_code = ${km.ps2_key.code}; return; // ${km.web_name}
% endfor
	}
}
