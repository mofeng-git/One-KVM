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


#include <Arduino.h>
#include <HID-Project.h>
#include <TimerOne.h>

#include "inline.h"
#include "keymap.h"


// #define CMD_SERIAL		Serial1
#define CMD_SERIAL_SPEED	115200
#define CMD_RECV_TIMEOUT	100000

#define PROTO_MAGIC						0x33
#define PROTO_CRC_POLINOM				0xA001
// -----------------------------------------
#define PROTO_RESP_OK					0x20
#define PROTO_RESP_NONE					0x24
#define PROTO_RESP_CRC_ERROR			0x40
#define PROTO_RESP_INVALID_ERROR		0x45
#define PROTO_RESP_TIMEOUT_ERROR		0x48
// -----------------------------------------
#define PROTO_CMD_PING					0x01
#define PROTO_CMD_REPEAT				0x02
#define PROTO_CMD_RESET_HID				0x10
#define PROTO_CMD_KEY_EVENT				0x11
#define PROTO_CMD_MOUSE_MOVE_EVENT		0x12
#define PROTO_CMD_MOUSE_BUTTON_EVENT	0x13
#define PROTO_CMD_MOUSE_WHEEL_EVENT		0x14
// -----------------------------------------
#define PROTO_CMD_MOUSE_BUTTON_LEFT_SELECT	0b10000000
#define PROTO_CMD_MOUSE_BUTTON_LEFT_STATE	0b00001000
#define PROTO_CMD_MOUSE_BUTTON_RIGHT_SELECT	0b01000000
#define PROTO_CMD_MOUSE_BUTTON_RIGHT_STATE	0b00000100


// -----------------------------------------------------------------------------
INLINE void cmdResetHid(const uint8_t *buffer) { // 0 bytes
	BootKeyboard.releaseAll();
	SingleAbsoluteMouse.releaseAll();
}

INLINE void cmdKeyEvent(const uint8_t *buffer) { // 2 bytes
	KeyboardKeycode code = keymap(buffer[0]);

	if (code != KEY_ERROR_UNDEFINED) {
		if (buffer[1]) {
			BootKeyboard.press(code);
		} else {
			BootKeyboard.release(code);
		}
	}
}

INLINE void cmdMouseMoveEvent(const uint8_t *buffer) { // 4 bytes
	int x = (int)buffer[0] << 8;
	x |= (int)buffer[1];

	int y = (int)buffer[2] << 8;
	y |= (int)buffer[3];

	SingleAbsoluteMouse.moveTo(x, y);
}

INLINE void cmdMouseButtonEvent(const uint8_t *buffer) { // 1 byte
	uint8_t state = buffer[0];

	if (state & PROTO_CMD_MOUSE_BUTTON_LEFT_SELECT) {
		if (state & PROTO_CMD_MOUSE_BUTTON_LEFT_STATE) {
			SingleAbsoluteMouse.press(MOUSE_LEFT);
		} else {
			SingleAbsoluteMouse.release(MOUSE_LEFT);
		}
	}

	if (state & PROTO_CMD_MOUSE_BUTTON_RIGHT_SELECT) {
		if (state & PROTO_CMD_MOUSE_BUTTON_RIGHT_STATE) {
			SingleAbsoluteMouse.press(MOUSE_RIGHT);
		} else {
			SingleAbsoluteMouse.release(MOUSE_RIGHT);
		}
	}
}

INLINE void cmdMouseWheelEvent(const uint8_t *buffer) { // 2 bytes
	// delta_x is not supported by hid-project now
	signed char delta_y = buffer[1];

	SingleAbsoluteMouse.move(0, 0, delta_y);
}


// -----------------------------------------------------------------------------
INLINE uint16_t makeCrc16(const uint8_t *buffer, const unsigned length) {
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


// -----------------------------------------------------------------------------
volatile bool cmd_recv_timed_out = false;

INLINE void recvTimerStop(bool flag) {
	Timer1.stop();
	cmd_recv_timed_out = flag;
}

INLINE void resetCmdRecvTimeout() {
	recvTimerStop(false);
	Timer1.initialize(CMD_RECV_TIMEOUT);
}

INLINE void sendCmdResponse(uint8_t code=0) {
	static uint8_t prev_code = PROTO_RESP_NONE;
	if (code == 0) {
		code = prev_code; // Repeat the last code
	} else {
		prev_code = code;
	}

	uint8_t buffer[4];
	buffer[0] = PROTO_MAGIC;
	buffer[1] = code;
	uint16_t crc = makeCrc16(buffer, 2);
	buffer[2] = (uint8_t)(crc >> 8);
	buffer[3] = (uint8_t)(crc & 0xFF);

	recvTimerStop(false);
	CMD_SERIAL.write(buffer, 4);
}

void intRecvTimedOut() {
	recvTimerStop(true);
}

void setup() {
	BootKeyboard.begin();
	SingleAbsoluteMouse.begin();

	Timer1.attachInterrupt(intRecvTimedOut);
	CMD_SERIAL.begin(CMD_SERIAL_SPEED);
}

void loop() {
	uint8_t buffer[8];
	unsigned index = 0;

	while (true) {
		if (CMD_SERIAL.available() > 0) {
			buffer[index] = (uint8_t)CMD_SERIAL.read();
			if (index == 7) {
				uint16_t crc = (uint16_t)buffer[6] << 8;
				crc |= (uint16_t)buffer[7];

				if (makeCrc16(buffer, 6) == crc) {
#	define HANDLE(_handler) { _handler(buffer + 2); sendCmdResponse(PROTO_RESP_OK); break; }
					switch (buffer[1]) {
						case PROTO_CMD_RESET_HID:			HANDLE(cmdResetHid);
						case PROTO_CMD_KEY_EVENT:			HANDLE(cmdKeyEvent);
						case PROTO_CMD_MOUSE_MOVE_EVENT:	HANDLE(cmdMouseMoveEvent);
						case PROTO_CMD_MOUSE_BUTTON_EVENT:	HANDLE(cmdMouseButtonEvent);
						case PROTO_CMD_MOUSE_WHEEL_EVENT:	HANDLE(cmdMouseWheelEvent);

						case PROTO_CMD_PING:	sendCmdResponse(PROTO_RESP_OK); break;
						case PROTO_CMD_REPEAT:	sendCmdResponse(); break;
						default:				sendCmdResponse(PROTO_RESP_INVALID_ERROR); break;
					}
#	undef HANDLE
				} else {
					sendCmdResponse(PROTO_RESP_CRC_ERROR);
				}
				index = 0;
			} else {
				resetCmdRecvTimeout();
				index += 1;
			}
		} else if (index > 0 && cmd_recv_timed_out) {
			sendCmdResponse(PROTO_RESP_TIMEOUT_ERROR);
			index = 0;
		}
	}
}
