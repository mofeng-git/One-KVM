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


#include "usb/hid.h"
#include "ps2/hid.h"
#include "factory.h"
#include "eeprom.h"
#include "serial.h"
#include "spi.h"

#ifndef ARDUINO_ARCH_AVR
#	error "Only AVR is supported"
#endif


namespace DRIVERS {
	Keyboard *Factory::makeKeyboard(type _type) {
		switch (_type) {
#			ifdef HID_WITH_USB
			case USB_KEYBOARD:
				return new UsbKeyboard();
#			endif

#			ifdef HID_WITH_PS2
			case PS2_KEYBOARD:
				return new Ps2Keyboard();
#			endif

			default:
				return new Keyboard(DUMMY);
		}
	}

	Mouse *Factory::makeMouse(type _type) {
		switch (_type) {
#			ifdef HID_WITH_USB
			case USB_MOUSE_ABSOLUTE:
			case USB_MOUSE_ABSOLUTE_WIN98:
				return new UsbMouseAbsolute(_type);
			case USB_MOUSE_RELATIVE:
				return new UsbMouseRelative();
#			endif
			default:
				return new Mouse(DRIVERS::DUMMY);
		}
	}

	Storage *Factory::makeStorage(type _type) {
		switch (_type) {
#			ifdef HID_DYNAMIC
			case NON_VOLATILE_STORAGE:
				return new Eeprom(DRIVERS::NON_VOLATILE_STORAGE);
#			endif
			default:
				return new Storage(DRIVERS::DUMMY);
		}
	}

	Board *Factory::makeBoard(type _type) {
		switch (_type) {
			default:
				return new Board(DRIVERS::DUMMY);
		}
	}

	Connection *Factory::makeConnection(type _type) {
#		ifdef CMD_SERIAL
		return new Serial();
#		elif defined(CMD_SPI)
		return new Spi();
#		else
#		error CMD phy is not defined
#		endif		
	}
}
