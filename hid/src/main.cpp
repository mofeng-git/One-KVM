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
#include <TimerOne.h>

#include "inline.h"

#if defined(HID_USB_KBD) || defined(HID_USB_MOUSE)
#	include "usb/hid.h"
#endif
#ifdef HID_PS2_KBD
#	include "ps2/hid.h"
#endif


// #define CMD_SERIAL		Serial1
// #define CMD_SERIAL_SPEED	115200
#define CMD_RECV_TIMEOUT	100000

#define PROTO_MAGIC			0x33
#define PROTO_CRC_POLINOM	0xA001

#define PROTO_RESP_OK				0x20
#define PROTO_RESP_NONE				0x24
#define PROTO_RESP_CRC_ERROR		0x40
#define PROTO_RESP_INVALID_ERROR	0x45
#define PROTO_RESP_TIMEOUT_ERROR	0x48

#define PROTO_RESP_PONG_PREFIX	0x80
#define PROTO_RESP_PONG_CAPS	0b00000001
#define PROTO_RESP_PONG_SCROLL	0b00000010
#define PROTO_RESP_PONG_NUM		0b00000100

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


// -----------------------------------------------------------------------------
#ifdef HID_USB_KBD
	UsbHidKeyboard hid_kbd;
#elif defined(HID_PS2_KBD)
	Ps2HidKeyboard hid_kbd;
#endif
#ifdef HID_USB_MOUSE
	UsbHidMouse hid_mouse;
#endif


// -----------------------------------------------------------------------------
INLINE uint8_t cmdResetHid(const uint8_t *buffer) { // 0 bytes
#	ifdef HID_USB_KBD
	hid_kbd.reset();
#	endif
#	ifdef HID_USB_MOUSE
	hid_mouse.reset();
#	endif
	return PROTO_RESP_OK;
}

INLINE uint8_t cmdKeyEvent(const uint8_t *buffer) { // 2 bytes
	hid_kbd.sendKey(buffer[0], buffer[1]);
	return PROTO_RESP_OK;
}

INLINE uint8_t cmdMouseButtonEvent(const uint8_t *buffer) { // 2 bytes
#	ifdef HID_USB_MOUSE
	uint8_t main_state = buffer[0];
	uint8_t extra_state = buffer[1];

#	define MOUSE_PAIR(_state, _button) \
		_state & PROTO_CMD_MOUSE_BUTTON_##_button##_SELECT, \
		_state & PROTO_CMD_MOUSE_BUTTON_##_button##_STATE
	hid_mouse.sendMouseButtons(
		MOUSE_PAIR(main_state, LEFT),
		MOUSE_PAIR(main_state, RIGHT),
		MOUSE_PAIR(main_state, MIDDLE),
		MOUSE_PAIR(extra_state, EXTRA_UP),
		MOUSE_PAIR(extra_state, EXTRA_DOWN)
	);
#	undef MOUSE_PAIR
#	endif
	return PROTO_RESP_OK;
}

INLINE uint8_t cmdMouseMoveEvent(const uint8_t *buffer) { // 4 bytes
#	ifdef HID_USB_MOUSE
	int x = (int)buffer[0] << 8;
	x |= (int)buffer[1];
	x = (x + 32768) / 2; // See /kvmd/apps/otg/hid/keyboard.py for details

	int y = (int)buffer[2] << 8;
	y |= (int)buffer[3];
	y = (y + 32768) / 2; // See /kvmd/apps/otg/hid/keyboard.py for details

	hid_mouse.sendMouseMove(x, y);
#	endif
	return PROTO_RESP_OK;
}

INLINE uint8_t cmdMouseWheelEvent(const uint8_t *buffer) { // 2 bytes
#	ifdef HID_USB_MOUSE
	hid_mouse.sendMouseWheel(buffer[1]); // Y only, X is not supported
#	endif
	return PROTO_RESP_OK;
}

INLINE uint8_t cmdPongLeds(const uint8_t *buffer) { // 0 bytes
	return ((uint8_t) PROTO_RESP_PONG_PREFIX) | hid_kbd.getLedsAs(
		PROTO_RESP_PONG_CAPS,
		PROTO_RESP_PONG_SCROLL,
		PROTO_RESP_PONG_NUM
	);
}


// -----------------------------------------------------------------------------
INLINE uint16_t makeCrc16(const uint8_t *buffer, unsigned length) {
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
	hid_kbd.begin();
#	ifdef HID_USB_MOUSE
	hid_mouse.begin();
#	endif

	Timer1.attachInterrupt(intRecvTimedOut);
	CMD_SERIAL.begin(CMD_SERIAL_SPEED);
}

void loop() {
	uint8_t buffer[8];
	unsigned index = 0;

	while (true) {
#		ifdef HID_PS2_KBD
		hid_kbd.periodic();
#		endif

		if (CMD_SERIAL.available() > 0) {
			buffer[index] = (uint8_t)CMD_SERIAL.read();
			if (index == 7) {
				uint16_t crc = (uint16_t)buffer[6] << 8;
				crc |= (uint16_t)buffer[7];

				if (makeCrc16(buffer, 6) == crc) {
#					define HANDLE(_handler) { sendCmdResponse(_handler(buffer + 2)); break; }
					switch (buffer[1]) {
						case PROTO_CMD_RESET_HID:			HANDLE(cmdResetHid);
						case PROTO_CMD_KEY_EVENT:			HANDLE(cmdKeyEvent);
						case PROTO_CMD_MOUSE_BUTTON_EVENT:	HANDLE(cmdMouseButtonEvent);
						case PROTO_CMD_MOUSE_MOVE_EVENT:	HANDLE(cmdMouseMoveEvent);
						case PROTO_CMD_MOUSE_WHEEL_EVENT:	HANDLE(cmdMouseWheelEvent);
						case PROTO_CMD_PING:				HANDLE(cmdPongLeds);
						case PROTO_CMD_REPEAT:	sendCmdResponse(); break;
						default:				sendCmdResponse(PROTO_RESP_INVALID_ERROR); break;
					}
#					undef HANDLE
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
