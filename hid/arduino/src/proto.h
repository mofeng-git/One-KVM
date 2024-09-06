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


namespace PROTO {
	const uint8_t MAGIC			= 0x33;
	const uint8_t MAGIC_RESP	= 0x34;

	namespace RESP { // Plain responses
		// const uint8_t OK =			0x20; // Legacy
		const uint8_t NONE =			0x24;
		const uint8_t CRC_ERROR =		0x40;
		const uint8_t INVALID_ERROR =	0x45;
		const uint8_t TIMEOUT_ERROR =	0x48;
	};

	namespace PONG { // Complex response
		const uint8_t OK =					0x80;
		const uint8_t CAPS =				0b00000001;
		const uint8_t SCROLL =				0b00000010;
		const uint8_t NUM =					0b00000100;
		const uint8_t KEYBOARD_OFFLINE =	0b00001000;
		const uint8_t MOUSE_OFFLINE =		0b00010000;
		const uint8_t RESET_REQUIRED =		0b01000000;
	};

	namespace OUTPUTS1 { // Complex request/responce flags
		const uint8_t DYNAMIC =		0b10000000;
		namespace KEYBOARD {
			const uint8_t MASK =	0b00000111;
			const uint8_t USB =		0b00000001;
			const uint8_t PS2 =		0b00000011;
		};
		namespace MOUSE {
			const uint8_t MASK =		0b00111000;
			const uint8_t USB_ABS =		0b00001000;
			const uint8_t USB_REL =		0b00010000;
			const uint8_t PS2 =			0b00011000;
			const uint8_t USB_WIN98 =	0b00100000;
		};
	};

	namespace OUTPUTS2 { // Complex response
		const uint8_t CONNECTABLE =		0b10000000;
		const uint8_t CONNECTED =		0b01000000;
		const uint8_t HAS_USB =			0b00000001;
		const uint8_t HAS_PS2 =			0b00000010;
		const uint8_t HAS_USB_WIN98 =	0b00000100;
	}

	namespace CMD {
		const uint8_t PING =			0x01;
		const uint8_t REPEAT =			0x02;
		const uint8_t SET_KEYBOARD =	0x03;
		const uint8_t SET_MOUSE =		0x04;
		const uint8_t SET_CONNECTED =	0x05;
		const uint8_t CLEAR_HID =		0x10;

		namespace KEYBOARD {
			const uint8_t KEY =	0x11;
		};

		namespace MOUSE {
			const uint8_t MOVE =		0x12;
			const uint8_t BUTTON =		0x13;
			const uint8_t WHEEL =		0x14;
			const uint8_t RELATIVE =	0x15;
			namespace LEFT {
				const uint8_t SELECT =	0b10000000;
				const uint8_t STATE =	0b00001000;
			};
			namespace RIGHT {
				const uint8_t SELECT =	0b01000000;
				const uint8_t STATE =	0b00000100;
			};
			namespace MIDDLE {
				const uint8_t SELECT =	0b00100000;
				const uint8_t STATE =	0b00000010;
			};
			namespace EXTRA_UP {
				const uint8_t SELECT =	0b10000000;
				const uint8_t STATE =	0b00001000;
			};
			namespace EXTRA_DOWN {
				const uint8_t SELECT =	0b01000000;
				const uint8_t STATE =	0b00000100;
			};
		};
	};

	uint16_t crc16(const uint8_t *buffer, unsigned length) {
		const uint16_t polinom = 0xA001;
		uint16_t crc = 0xFFFF;

		for (unsigned byte_count = 0; byte_count < length; ++byte_count) {
			crc = crc ^ buffer[byte_count];
			for (unsigned bit_count = 0; bit_count < 8; ++bit_count) {
				if ((crc & 0x0001) == 0) {
					crc = crc >> 1;
				} else {
					crc = crc >> 1;
					crc = crc ^ polinom;
				}
			}
		}
		return crc;
	}

	inline int merge8_int(uint8_t from_a, uint8_t from_b) {
		return (((int)from_a << 8) | (int)from_b);
	}

	inline uint16_t merge8(uint8_t from_a, uint8_t from_b) {
		return (((uint16_t)from_a << 8) | (uint16_t)from_b);
	}

	inline void split16(uint16_t from, uint8_t *to_a, uint8_t *to_b) {
		*to_a = (uint8_t)(from >> 8);
		*to_b = (uint8_t)(from & 0xFF);
	}
};
