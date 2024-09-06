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


#include "factory.h"
#include "proto.h"


class Outputs {
	public:
		void writeOutputs(uint8_t mask, uint8_t outputs, bool force) {
			int old = 0;
			if (!force) {
				old = _readOutputs();
				if (old < 0) {
					old = 0;
				}
			}
			uint8_t data[8] = {0};
			data[0] = PROTO::MAGIC;
			data[1] = (old & ~mask) | outputs;
			PROTO::split16(PROTO::crc16(data, 6), &data[6], &data[7]);
			_storage->updateBlock(data, 0, 8);
		}

		void initOutputs() {
#			ifdef HID_DYNAMIC
			_storage = DRIVERS::Factory::makeStorage(DRIVERS::NON_VOLATILE_STORAGE);
#			else
			_storage = DRIVERS::Factory::makeStorage(DRIVERS::DUMMY);
#			endif

			int outputs = _readOutputs();
			if (outputs < 0) {
				outputs = 0;
#				if defined(HID_WITH_USB) && defined(HID_SET_USB_KBD)
				outputs |= PROTO::OUTPUTS1::KEYBOARD::USB;
#				elif defined(HID_WITH_PS2) && defined(HID_SET_PS2_KBD)
				outputs |= PROTO::OUTPUTS1::KEYBOARD::PS2;
#				endif
#				if defined(HID_WITH_USB) && defined(HID_SET_USB_MOUSE_ABS)
				outputs |= PROTO::OUTPUTS1::MOUSE::USB_ABS;
#				elif defined(HID_WITH_USB) && defined(HID_SET_USB_MOUSE_REL)
				outputs |= PROTO::OUTPUTS1::MOUSE::USB_REL;
#				elif defined(HID_WITH_PS2) && defined(HID_SET_PS2_MOUSE)
				outputs |= PROTO::OUTPUTS1::MOUSE::PS2;
#				elif defined(HID_WITH_USB) && defined(HID_WITH_USB_WIN98) && defined(HID_SET_USB_MOUSE_WIN98)
				outputs |= PROTO::OUTPUTS1::MOUSE::USB_WIN98;
#				endif
				writeOutputs(0xFF, outputs, true);
			}

			uint8_t kbd_type = outputs & PROTO::OUTPUTS1::KEYBOARD::MASK;
			switch (kbd_type) {
				case PROTO::OUTPUTS1::KEYBOARD::USB:
					kbd = DRIVERS::Factory::makeKeyboard(DRIVERS::USB_KEYBOARD);
					break;
				case PROTO::OUTPUTS1::KEYBOARD::PS2:
					kbd = DRIVERS::Factory::makeKeyboard(DRIVERS::PS2_KEYBOARD);
					break;
				default:
					kbd = DRIVERS::Factory::makeKeyboard(DRIVERS::DUMMY);
					break;
			}

			uint8_t mouse_type = outputs & PROTO::OUTPUTS1::MOUSE::MASK;
			switch (mouse_type) {
				case PROTO::OUTPUTS1::MOUSE::USB_ABS:
					mouse = DRIVERS::Factory::makeMouse(DRIVERS::USB_MOUSE_ABSOLUTE);
					break;
				case PROTO::OUTPUTS1::MOUSE::USB_WIN98:
					mouse = DRIVERS::Factory::makeMouse(DRIVERS::USB_MOUSE_ABSOLUTE_WIN98);
					break;
				case PROTO::OUTPUTS1::MOUSE::USB_REL:
					mouse = DRIVERS::Factory::makeMouse(DRIVERS::USB_MOUSE_RELATIVE);
					break;
				default:
					mouse = DRIVERS::Factory::makeMouse(DRIVERS::DUMMY);
					break;
			}

#			ifdef ARDUINO_ARCH_AVR
			USBDevice.attach();
#			endif

			kbd->begin();
			mouse->begin();
		}

		DRIVERS::Keyboard *kbd = nullptr;
		DRIVERS::Mouse *mouse = nullptr;
		
	private:
		int _readOutputs(void) {
			uint8_t data[8];
			_storage->readBlock(data, 0, 8);
			if (data[0] != PROTO::MAGIC || PROTO::crc16(data, 6) != PROTO::merge8(data[6], data[7])) {
				return -1;
			}
			return data[1];
		}

		DRIVERS::Storage *_storage = nullptr;
};
