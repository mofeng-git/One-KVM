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


#if !(defined(CMD_SERIAL) || defined(CMD_SPI))
#	error CMD phy is not defined
#endif


#include <Arduino.h>
#ifdef CMD_SPI
#	include <SPI.h>
#endif

#include "proto.h"

#if defined(HID_USB_KBD) || defined(HID_USB_MOUSE)
#	include "usb/hid.h"
#endif
#ifdef HID_PS2_KBD
#	include "ps2/hid.h"
#endif


// #define CMD_SERIAL			Serial1
// #define CMD_SERIAL_SPEED		115200
// #define CMD_SERIAL_TIMEOUT	100000
// -- OR --
// #define CMD_SPI


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
uint8_t cmdResetHid(const uint8_t *buffer) { // 0 bytes
#	ifdef HID_USB_KBD
	hid_kbd.reset();
#	endif
#	ifdef HID_USB_MOUSE
	hid_mouse.reset();
#	endif
	return PROTO_RESP_OK;
}

uint8_t cmdKeyEvent(const uint8_t *buffer) { // 2 bytes
	hid_kbd.sendKey(buffer[0], buffer[1]);
	return PROTO_RESP_OK;
}

uint8_t cmdMouseButtonEvent(const uint8_t *buffer) { // 2 bytes
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

uint8_t cmdMouseMoveEvent(const uint8_t *buffer) { // 4 bytes
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

uint8_t cmdMouseWheelEvent(const uint8_t *buffer) { // 2 bytes
#	ifdef HID_USB_MOUSE
	hid_mouse.sendMouseWheel(buffer[1]); // Y only, X is not supported
#	endif
	return PROTO_RESP_OK;
}

uint8_t cmdPongLeds(const uint8_t *buffer) { // 0 bytes
	return ((uint8_t) PROTO_RESP_PONG_PREFIX) | hid_kbd.getLedsAs(
		PROTO_RESP_PONG_CAPS,
		PROTO_RESP_PONG_SCROLL,
		PROTO_RESP_PONG_NUM
	);
}

uint8_t handleCmdBuffer(const uint8_t *buffer) { // 8 bytes
	uint16_t crc = (uint16_t)buffer[6] << 8;
	crc |= (uint16_t)buffer[7];

	if (protoCrc16(buffer, 6) == crc) {
#		define HANDLE(_handler) { return _handler(buffer + 2); }
		switch (buffer[1]) {
			case PROTO_CMD_RESET_HID:			HANDLE(cmdResetHid);
			case PROTO_CMD_KEY_EVENT:			HANDLE(cmdKeyEvent);
			case PROTO_CMD_MOUSE_BUTTON_EVENT:	HANDLE(cmdMouseButtonEvent);
			case PROTO_CMD_MOUSE_MOVE_EVENT:	HANDLE(cmdMouseMoveEvent);
			case PROTO_CMD_MOUSE_WHEEL_EVENT:	HANDLE(cmdMouseWheelEvent);
			case PROTO_CMD_PING:				HANDLE(cmdPongLeds);
			case PROTO_CMD_REPEAT:	return 0;
			default:				return PROTO_RESP_INVALID_ERROR;
		}
#		undef HANDLE
	}
	return PROTO_RESP_CRC_ERROR;
}


// -----------------------------------------------------------------------------
#ifdef CMD_SPI
volatile uint8_t spi_in[8] = {0};
volatile uint8_t spi_in_index = 0;

volatile uint8_t spi_out[4] = {0};
volatile uint8_t spi_out_index = 0;

bool spiReady() {
	return (!spi_out[0] && spi_in_index == 8);
}

void spiWrite(const uint8_t *buffer) {
	spi_out[3] = buffer[3];
	spi_out[2] = buffer[2];
	spi_out[1] = buffer[1];
	spi_out[0] = buffer[0]; // Меджик разрешает начать ответ
//	digitalWrite(5, 1);
}

ISR(SPI_STC_vect) {
	uint8_t in = SPDR;
	if (spi_out[0] && spi_out_index < 4) {
//		digitalWrite(4, !digitalRead(4));
		SPDR = spi_out[spi_out_index];
		bool err = (SPSR & (1 << WCOL));
		if (!err) {
			++spi_out_index;
			if (spi_out_index == 4) {
				spi_out_index = 0;
				spi_in_index = 0;
				spi_out[0] = 0;
//				digitalWrite(5, 0);
			}
		}
	} else {
		static bool receiving = false;
		if (!receiving && in == PROTO_MAGIC) {
			receiving = true;
		}
		if (receiving && spi_in_index < 8) {
			spi_in[spi_in_index] = in;
			++spi_in_index;
		}
		if (spi_in_index == 8) {
			receiving = false;
		}
		SPDR = 0;
	}
}
#endif


// -----------------------------------------------------------------------------
void sendCmdResponse(uint8_t code) {
	static uint8_t prev_code = PROTO_RESP_NONE;
	if (code == 0) {
		code = prev_code; // Repeat the last code
	} else {
		prev_code = code;
	}

	uint8_t buffer[4];
	buffer[0] = PROTO_MAGIC;
	buffer[1] = code;
	uint16_t crc = protoCrc16(buffer, 2);
	buffer[2] = (uint8_t)(crc >> 8);
	buffer[3] = (uint8_t)(crc & 0xFF);

#	ifdef CMD_SERIAL
	CMD_SERIAL.write(buffer, 4);
#	elif defined(CMD_SPI)
	spiWrite(buffer);
#	endif
}

void setup() {
	hid_kbd.begin();
#	ifdef HID_USB_MOUSE
	hid_mouse.begin();
#	endif

	pinMode(3, OUTPUT);
	pinMode(4, OUTPUT);
	pinMode(5, OUTPUT);
	pinMode(6, OUTPUT);

#	ifdef CMD_SERIAL
	CMD_SERIAL.begin(CMD_SERIAL_SPEED);
#	elif defined(CMD_SPI)
	pinMode(MISO, OUTPUT);
	SPCR = (1 << SPE) | (1 << SPIE); // Slave, SPI En, IRQ En
#	endif
}

void loop() {
#	ifdef CMD_SERIAL
	unsigned long last = micros();
	uint8_t buffer[8];
	uint8_t index = 0;
#	endif

	while (true) {
#		ifdef HID_PS2_KBD
		hid_kbd.periodic();
#		endif

#		ifdef CMD_SERIAL
		if (CMD_SERIAL.available() > 0) {
			buffer[index] = (uint8_t)CMD_SERIAL.read();
			if (index == 7) {
				sendCmdResponse(handleCmdBuffer(buffer));
				index = 0;
			} else {
				last = micros();
				++index;
			}
		} else if (index > 0) {
			unsigned long now = micros();
			if (
				(now >= last && now - last > CMD_SERIAL_TIMEOUT)
				|| (now < last && ((unsigned long)-1) - last + now > CMD_SERIAL_TIMEOUT)
			) {
				sendCmdResponse(PROTO_RESP_TIMEOUT_ERROR);
				index = 0;
			}
		}
#		elif defined(CMD_SPI)
		if (SPSR & (1 << WCOL)) {
			digitalWrite(3, HIGH);
			uint8_t _ = SPDR;
			delay(1);
			digitalWrite(3, LOW);
		}
		if (spiReady()) {
			sendCmdResponse(handleCmdBuffer(spi_in));
		}
#		endif
	}
}
