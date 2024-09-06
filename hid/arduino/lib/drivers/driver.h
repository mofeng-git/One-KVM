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

#include <stdint.h>


namespace DRIVERS {
	enum type {
		DUMMY = 0,
		USB_MOUSE_ABSOLUTE,
		USB_MOUSE_RELATIVE,
		USB_MOUSE_ABSOLUTE_WIN98,
		USB_KEYBOARD,
		PS2_KEYBOARD,
		NON_VOLATILE_STORAGE,
		BOARD,
		CONNECTION,
	};

	class Driver {
	public:
		Driver(type _type) : _type(_type) {}
		uint8_t getType() { return _type; }

	private:
		type _type;
	};
}
