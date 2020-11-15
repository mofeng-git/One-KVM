/*****************************************************************************
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
*****************************************************************************/


#pragma once


#define PROTO_MAGIC			0x33
#define PROTO_CRC_POLINOM	0xA001

// #define PROTO_RESP_OK			0x20 // Legacy
#define PROTO_RESP_NONE				0x24
#define PROTO_RESP_CRC_ERROR		0x40
#define PROTO_RESP_INVALID_ERROR	0x45
#define PROTO_RESP_TIMEOUT_ERROR	0x48

#define PROTO_RESP_PONG_PREFIX				0x80
#define PROTO_RESP_PONG_CAPS				0b00000001
#define PROTO_RESP_PONG_SCROLL				0b00000010
#define PROTO_RESP_PONG_NUM					0b00000100
#define PROTO_RESP_PONG_KEYBOARD_OFFLINE	0b00001000
#define PROTO_RESP_PONG_MOUSE_OFFLINE		0b00010000

#define PROTO_CMD_PING					0x01
#define PROTO_CMD_REPEAT				0x02
#define PROTO_CMD_RESET_HID				0x10
#define PROTO_CMD_KEY_EVENT				0x11
#define PROTO_CMD_MOUSE_BUTTON_EVENT	0x13 // Legacy sequence
#define PROTO_CMD_MOUSE_MOVE_EVENT		0x12
#define PROTO_CMD_MOUSE_WHEEL_EVENT		0x14

#define PROTO_CMD_MOUSE_BUTTON_LEFT_SELECT		0b10000000
#define PROTO_CMD_MOUSE_BUTTON_LEFT_STATE		0b00001000
#define PROTO_CMD_MOUSE_BUTTON_RIGHT_SELECT		0b01000000
#define PROTO_CMD_MOUSE_BUTTON_RIGHT_STATE		0b00000100
#define PROTO_CMD_MOUSE_BUTTON_MIDDLE_SELECT	0b00100000
#define PROTO_CMD_MOUSE_BUTTON_MIDDLE_STATE		0b00000010

#define PROTO_CMD_MOUSE_BUTTON_EXTRA_UP_SELECT		0b10000000
#define PROTO_CMD_MOUSE_BUTTON_EXTRA_UP_STATE		0b00001000
#define PROTO_CMD_MOUSE_BUTTON_EXTRA_DOWN_SELECT	0b01000000
#define PROTO_CMD_MOUSE_BUTTON_EXTRA_DOWN_STATE		0b00000100


uint16_t protoCrc16(const uint8_t *buffer, unsigned length) {
	uint16_t crc = 0xFFFF;

	for (unsigned byte_count = 0; byte_count < length; ++byte_count) {
		crc = crc ^ buffer[byte_count];
		for (unsigned bit_count = 0; bit_count < 8; ++bit_count) {
			if ((crc & 0x0001) == 0) {
				crc = crc >> 1;
			} else {
				crc = crc >> 1;
				crc = crc ^ PROTO_CRC_POLINOM;
			}
		}
	}
	return crc;
}
