/*****************************************************************************
#                                                                            #
#    KVMD - The main PiKVM daemon.                                           #
#                                                                            #
#    Copyright (C) 2018-2022  Maxim Devaev <mdevaev@gmail.com>               #
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


// #define CMD_SERIAL			Serial1
// #define CMD_SERIAL_SPEED		115200
// #define CMD_SERIAL_TIMEOUT	100000
// -- OR --
// #define CMD_SPI

#if !(defined(CMD_SERIAL) || defined(CMD_SPI))
#	error CMD phy is not defined
#endif


#include <Arduino.h>

#include "tools.h"
#include "proto.h"
#ifdef CMD_SPI
#	include "spi.h"
#endif
#ifdef AUM
#	include "aum.h"
#endif
#include "board.h"
#include "outputs.h"

static DRIVERS::Board* _board;
static Outputs _out;
#ifdef HID_DYNAMIC
static bool _reset_required = false;
static unsigned long _reset_timestamp;
#	define RESET_TIMEOUT 500000
#endif


// -----------------------------------------------------------------------------
#ifdef HID_DYNAMIC
static void _resetRequest() {
	_reset_required = true;
	_reset_timestamp = micros();
}
#endif

static void _cmdSetKeyboard(const uint8_t *data) { // 1 bytes
#	ifdef HID_DYNAMIC
	_out.writeOutputs(PROTO::OUTPUTS1::KEYBOARD::MASK, data[0], false);
	_resetRequest();
#	endif
}

static void _cmdSetMouse(const uint8_t *data) { // 1 bytes
#	ifdef HID_DYNAMIC
	_out.writeOutputs(PROTO::OUTPUTS1::MOUSE::MASK, data[0], false);
	_resetRequest();
#	endif
}

static void _cmdSetConnected(const uint8_t *data) { // 1 byte
#	ifdef AUM
	aumSetUsbConnected(data[0]);
#	endif
}

static void _cmdClearHid(const uint8_t *_) { // 0 bytes
	_out.kbd->clear();
	_out.mouse->clear();
}

static void _cmdKeyEvent(const uint8_t *data) { // 2 bytes
	_out.kbd->sendKey(data[0], data[1]);
}

static void _cmdMouseButtonEvent(const uint8_t *data) { // 2 bytes
#	define MOUSE_PAIR(_state, _button) \
		_state & PROTO::CMD::MOUSE::_button::SELECT, \
		_state & PROTO::CMD::MOUSE::_button::STATE
	_out.mouse->sendButtons(
		MOUSE_PAIR(data[0], LEFT),
		MOUSE_PAIR(data[0], RIGHT),
		MOUSE_PAIR(data[0], MIDDLE),
		MOUSE_PAIR(data[1], EXTRA_UP),
		MOUSE_PAIR(data[1], EXTRA_DOWN)
	);
#	undef MOUSE_PAIR
}

static void _cmdMouseMoveEvent(const uint8_t *data) { // 4 bytes
	// See /kvmd/apps/otg/hid/keyboard.py for details
	_out.mouse->sendMove(
		PROTO::merge8_int(data[0], data[1]),
		PROTO::merge8_int(data[2], data[3])
	);
}

static void _cmdMouseRelativeEvent(const uint8_t *data) { // 2 bytes
	_out.mouse->sendRelative(data[0], data[1]);
}

static void _cmdMouseWheelEvent(const uint8_t *data) { // 2 bytes
	// Y only, X is not supported
	_out.mouse->sendWheel(data[1]);
}

static uint8_t _handleRequest(const uint8_t *data) { // 8 bytes
	_board->updateStatus(DRIVERS::RX_DATA);
	if (PROTO::crc16(data, 6) == PROTO::merge8(data[6], data[7])) {
#		define HANDLE(_handler) { _handler(data + 2); return PROTO::PONG::OK; }
		switch (data[1]) {
			case PROTO::CMD::PING:				return PROTO::PONG::OK;
			case PROTO::CMD::SET_KEYBOARD:		HANDLE(_cmdSetKeyboard);
			case PROTO::CMD::SET_MOUSE:			HANDLE(_cmdSetMouse);
			case PROTO::CMD::SET_CONNECTED:		HANDLE(_cmdSetConnected);
			case PROTO::CMD::CLEAR_HID:			HANDLE(_cmdClearHid);
			case PROTO::CMD::KEYBOARD::KEY:		HANDLE(_cmdKeyEvent);
			case PROTO::CMD::MOUSE::BUTTON:		HANDLE(_cmdMouseButtonEvent);
			case PROTO::CMD::MOUSE::MOVE:		HANDLE(_cmdMouseMoveEvent);
			case PROTO::CMD::MOUSE::RELATIVE:	HANDLE(_cmdMouseRelativeEvent);
			case PROTO::CMD::MOUSE::WHEEL:		HANDLE(_cmdMouseWheelEvent);
			case PROTO::CMD::REPEAT:	return 0;
			default:					return PROTO::RESP::INVALID_ERROR;
		}
#		undef HANDLE
	}
	return PROTO::RESP::CRC_ERROR;
}


// -----------------------------------------------------------------------------
static void _sendResponse(uint8_t code) {
	static uint8_t prev_code = PROTO::RESP::NONE;
	if (code == 0) {
		code = prev_code; // Repeat the last code
	} else {
		prev_code = code;
	}

	uint8_t response[8] = {0};
	response[0] = PROTO::MAGIC_RESP;
	if (code & PROTO::PONG::OK) {
		response[1] = PROTO::PONG::OK;
#		ifdef HID_DYNAMIC
		if (_reset_required) {
			response[1] |= PROTO::PONG::RESET_REQUIRED;
			if (is_micros_timed_out(_reset_timestamp, RESET_TIMEOUT)) {
				_board->reset();
			}
		}
		response[2] = PROTO::OUTPUTS1::DYNAMIC;
#		endif
		if (_out.kbd->getType() != DRIVERS::DUMMY) {
			if(_out.kbd->isOffline()) {
				response[1] |= PROTO::PONG::KEYBOARD_OFFLINE;
			} else {
				_board->updateStatus(DRIVERS::KEYBOARD_ONLINE);
			}
			DRIVERS::KeyboardLedsState leds = _out.kbd->getLeds();
			response[1] |= (leds.caps ? PROTO::PONG::CAPS : 0);
			response[1] |= (leds.num ? PROTO::PONG::NUM : 0);
			response[1] |= (leds.scroll ? PROTO::PONG::SCROLL : 0);
			switch (_out.kbd->getType()) {
				case DRIVERS::USB_KEYBOARD:
					response[2] |= PROTO::OUTPUTS1::KEYBOARD::USB;
					break;			
				case DRIVERS::PS2_KEYBOARD:
					response[2] |= PROTO::OUTPUTS1::KEYBOARD::PS2;
					break;			
			}	
		}
		if (_out.mouse->getType() != DRIVERS::DUMMY) {
			if(_out.mouse->isOffline()) {
				response[1] |= PROTO::PONG::MOUSE_OFFLINE;
			} else {
				_board->updateStatus(DRIVERS::MOUSE_ONLINE);
			}
			switch (_out.mouse->getType()) {
				case DRIVERS::USB_MOUSE_ABSOLUTE_WIN98:
					response[2] |= PROTO::OUTPUTS1::MOUSE::USB_WIN98;
					break;
				case DRIVERS::USB_MOUSE_ABSOLUTE:
					response[2] |= PROTO::OUTPUTS1::MOUSE::USB_ABS;
					break;
				case DRIVERS::USB_MOUSE_RELATIVE:
					response[2] |= PROTO::OUTPUTS1::MOUSE::USB_REL;
					break;
			}
		} // TODO: ps2
#		ifdef AUM
		response[3] |= PROTO::OUTPUTS2::CONNECTABLE;
		if (aumIsUsbConnected()) {
			response[3] |= PROTO::OUTPUTS2::CONNECTED;
		}
#		endif
#		ifdef HID_WITH_USB
		response[3] |= PROTO::OUTPUTS2::HAS_USB;
#		ifdef HID_WITH_USB_WIN98
		response[3] |= PROTO::OUTPUTS2::HAS_USB_WIN98;
#		endif
#		endif
#		ifdef HID_WITH_PS2
		response[3] |= PROTO::OUTPUTS2::HAS_PS2;
#		endif
	} else {
		response[1] = code;
	}
	PROTO::split16(PROTO::crc16(response, 6), &response[6], &response[7]);

#	ifdef CMD_SERIAL
	CMD_SERIAL.write(response, 8);
#	elif defined(CMD_SPI)
	spiWrite(response);
#	endif
}

void setup() {
	_out.initOutputs();

#	ifdef AUM
	aumInit();
#	endif

#	ifdef CMD_SERIAL
	CMD_SERIAL.begin(CMD_SERIAL_SPEED);
#	elif defined(CMD_SPI)
	spiBegin();
#	endif
	_board = DRIVERS::Factory::makeBoard(DRIVERS::BOARD);
}

void loop() {
#	ifdef AUM
	aumProxyUsbVbus();
#	endif

	_out.kbd->periodic();
	_out.mouse->periodic();
	_board->periodic();

#	ifdef CMD_SERIAL
	static unsigned long last = micros();
	static uint8_t buffer[8];
	static uint8_t index = 0;
	if (CMD_SERIAL.available() > 0) {
		buffer[index] = (uint8_t)CMD_SERIAL.read();
		if (index == 7) {
			_sendResponse(_handleRequest(buffer));
			index = 0;
		} else /*if (buffer[0] == PROTO::MAGIC)*/ { // FIXME: See kvmd/kvmd#80
			last = micros();
			++index;
		}
	} else if (index > 0) {
		if (is_micros_timed_out(last, CMD_SERIAL_TIMEOUT)) {
			_sendResponse(PROTO::RESP::TIMEOUT_ERROR);
			index = 0;
		}
	}
#	elif defined(CMD_SPI)
	if (spiReady()) {
		_sendResponse(_handleRequest(spiGet()));
	}
#	endif
}
