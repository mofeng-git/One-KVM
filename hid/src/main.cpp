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
#ifdef HID_DYNAMIC
#	include <avr/eeprom.h>
#endif

#include "proto.h"
#include "usb/hid.h"
#include "ps2/hid.h"


// #define CMD_SERIAL			Serial1
// #define CMD_SERIAL_SPEED		115200
// #define CMD_SERIAL_TIMEOUT	100000
// -- OR --
// #define CMD_SPI


// -----------------------------------------------------------------------------
static UsbKeyboard *_usb_kbd = NULL;
static UsbMouseAbsolute *_usb_mouse_abs = NULL;
static UsbMouseRelative *_usb_mouse_rel = NULL;

static Ps2Keyboard *_ps2_kbd = NULL;

#ifdef HID_DYNAMIC
static bool _reset_required = false;

static void _setOutputs(uint8_t outputs) {
	uint8_t data[8] = {0};
	data[0] = PROTO::MAGIC;
	data[1] = outputs;
	PROTO::split16(PROTO::crc16(data, 6), &data[6], &data[7]);
	eeprom_update_block(data, 0, 8);
}
#endif

static void _initOutputs() {
	uint8_t data[8];
#	ifdef HID_DYNAMIC
	eeprom_read_block(data, 0, 8);
	if (
		PROTO::crc16(data, 6) != PROTO::merge8(data[6], data[7])
		|| data[0] != PROTO::MAGIC
	) {
#	endif
		data[1] = 0;

#	if defined(HID_WITH_USB) && defined(HID_SET_USB_KBD)
		data[1] |= PROTO::OUTPUTS::KEYBOARD::USB;
#	elif defined(HID_WITH_PS2) && defined(HID_SET_PS2_KBD)
		data[1] |= PROTO::OUTPUTS::KEYBOARD::PS2;
#	endif
#	if defined(HID_WITH_USB) && defined(HID_SET_USB_MOUSE_ABS)
		data[1] |= PROTO::OUTPUTS::MOUSE::USB_ABS;
#	elif defined(HID_WITH_USB) && defined(HID_SET_USB_MOUSE_REL)
		data[1] |= PROTO::OUTPUTS::MOUSE::USB_REL;
#	elif defined(HID_WITH_PS2) && defined(HID_SET_PS2_MOUSE)
		data[1] |= PROTO::OUTPUTS::MOUSE::PS2;
#	endif

#	ifdef HID_DYNAMIC
		_setOutputs(data[1]);
	}
#	endif

	uint8_t kbd = data[1] & PROTO::OUTPUTS::KEYBOARD::MASK;
	switch (kbd) {
#	ifdef HID_WITH_USB
		case PROTO::OUTPUTS::KEYBOARD::USB: _usb_kbd = new UsbKeyboard(); break;
#	endif
#	ifdef HID_WITH_PS2
		case PROTO::OUTPUTS::KEYBOARD::PS2: _ps2_kbd = new Ps2Keyboard(); break;
#	endif
	}

	uint8_t mouse = data[1] & PROTO::OUTPUTS::MOUSE::MASK;
	switch (mouse) {
#	ifdef HID_WITH_USB
		case PROTO::OUTPUTS::MOUSE::USB_ABS: _usb_mouse_abs = new UsbMouseAbsolute(); break;
		case PROTO::OUTPUTS::MOUSE::USB_REL: _usb_mouse_rel = new UsbMouseRelative(); break;
#	endif
	}

	USBDevice.attach();

	switch (kbd) {
#	ifdef HID_WITH_USB
		case PROTO::OUTPUTS::KEYBOARD::USB: _usb_kbd->begin(); break;
#	endif
#	ifdef HID_WITH_PS2
		case PROTO::OUTPUTS::KEYBOARD::PS2: _ps2_kbd->begin(); break;
#	endif
	}

	switch (mouse) {
#	ifdef HID_WITH_USB
		case PROTO::OUTPUTS::MOUSE::USB_ABS: _usb_mouse_abs->begin(); break;
		case PROTO::OUTPUTS::MOUSE::USB_REL: _usb_mouse_rel->begin(); break;
#	endif
	}
}


// -----------------------------------------------------------------------------
static void _cmdSetOutputs(const uint8_t *data) { // 1 bytes
#	ifdef HID_DYNAMIC
	_setOutputs(data[0]);
	_reset_required = true;
#	endif
}

static void _cmdClearHid(const uint8_t *_) { // 0 bytes
	if (_usb_kbd) {
		_usb_kbd->clear();
	}
	if (_usb_mouse_abs) {
		_usb_mouse_abs->clear();
	} else if (_usb_mouse_rel) {
		_usb_mouse_rel->clear();
	}
}

static void _cmdKeyEvent(const uint8_t *data) { // 2 bytes
	if (_usb_kbd) {
		_usb_kbd->sendKey(data[0], data[1]);
	} else if (_ps2_kbd) {
		_ps2_kbd->sendKey(data[0], data[1]);
	}
}

static void _cmdMouseButtonEvent(const uint8_t *data) { // 2 bytes
#	define MOUSE_PAIR(_state, _button) \
		_state & PROTO::CMD::MOUSE::_button::SELECT, \
		_state & PROTO::CMD::MOUSE::_button::STATE
#	define SEND_BUTTONS(_hid) \
		_hid->sendButtons( \
			MOUSE_PAIR(data[0], LEFT), \
			MOUSE_PAIR(data[0], RIGHT), \
			MOUSE_PAIR(data[0], MIDDLE), \
			MOUSE_PAIR(data[1], EXTRA_UP), \
			MOUSE_PAIR(data[1], EXTRA_DOWN) \
		);
	if (_usb_mouse_abs) {
		SEND_BUTTONS(_usb_mouse_abs);
	} else if (_usb_mouse_rel) {
		SEND_BUTTONS(_usb_mouse_rel);
	}
#	undef SEND_BUTTONS
#	undef MOUSE_PAIR
}

static void _cmdMouseMoveEvent(const uint8_t *data) { // 4 bytes
	// See /kvmd/apps/otg/hid/keyboard.py for details
	if (_usb_mouse_abs) {
		_usb_mouse_abs->sendMove(
			(PROTO::merge8_int(data[0], data[1]) + 32768) / 2,
			(PROTO::merge8_int(data[2], data[3]) + 32768) / 2
		);
	}
}

static void _cmdMouseRelativeEvent(const uint8_t *data) { // 2 bytes
	if (_usb_mouse_rel) {
		_usb_mouse_rel->sendRelative(data[0], data[1]);
	}
}

static void _cmdMouseWheelEvent(const uint8_t *data) { // 2 bytes
	// Y only, X is not supported
	if (_usb_mouse_abs) {
		_usb_mouse_abs->sendWheel(data[1]);
	} else if (_usb_mouse_rel) {
		_usb_mouse_rel->sendWheel(data[1]);
	}
}

static uint8_t _handleRequest(const uint8_t *data) { // 8 bytes
	if (PROTO::crc16(data, 6) == PROTO::merge8(data[6], data[7])) {
#		define HANDLE(_handler) { _handler(data + 2); return PROTO::PONG::OK; }
		switch (data[1]) {
			case PROTO::CMD::PING:				return PROTO::PONG::OK;
			case PROTO::CMD::SET_OUTPUTS:		HANDLE(_cmdSetOutputs);
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
#ifdef CMD_SPI
static volatile uint8_t _spi_in[8] = {0};
static volatile uint8_t _spi_in_index = 0;

static volatile uint8_t _spi_out[8] = {0};
static volatile uint8_t _spi_out_index = 0;

static bool _spiReady() {
	return (!_spi_out[0] && _spi_in_index == 8);
}

static void _spiWrite(const uint8_t *data) {
	// Меджик в нулевом байте разрешает начать ответ
	for (int index = 7; index >= 0; --index) {
		_spi_out[index] = data[index];
	}
}

ISR(SPI_STC_vect) {
	uint8_t in = SPDR;
	if (_spi_out[0] && _spi_out_index < 8) {
		SPDR = _spi_out[_spi_out_index];
		if (!(SPSR & (1 << WCOL))) {
			++_spi_out_index;
			if (_spi_out_index == 8) {
				_spi_out_index = 0;
				_spi_in_index = 0;
				_spi_out[0] = 0;
			}
		}
	} else {
		static bool receiving = false;
		if (!receiving && in == PROTO::MAGIC) {
			receiving = true;
		}
		if (receiving && _spi_in_index < 8) {
			_spi_in[_spi_in_index] = in;
			++_spi_in_index;
		}
		if (_spi_in_index == 8) {
			receiving = false;
		}
		SPDR = 0;
	}
}
#endif


// -----------------------------------------------------------------------------
static void _sendResponse(uint8_t code) {
	static uint8_t prev_code = PROTO::RESP::NONE;
	if (code == 0) {
		code = prev_code; // Repeat the last code
	} else {
		prev_code = code;
	}

	uint8_t data[8] = {0};
	data[0] = PROTO::MAGIC;
	if (code & PROTO::PONG::OK) {
		data[1] = PROTO::PONG::OK;
#		ifdef HID_DYNAMIC
		if (_reset_required) {
			data[1] |= PROTO::PONG::RESET_REQUIRED;
		}
		data[2] = PROTO::OUTPUTS::DYNAMIC;
#		endif
		if (_usb_kbd) {
			data[1] |= _usb_kbd->getOfflineAs(PROTO::PONG::KEYBOARD_OFFLINE);
			data[1] |= _usb_kbd->getLedsAs(PROTO::PONG::CAPS, PROTO::PONG::SCROLL, PROTO::PONG::NUM);
			data[2] |= PROTO::OUTPUTS::KEYBOARD::USB;
		} else if (_ps2_kbd) {
			data[1] |= _ps2_kbd->getOfflineAs(PROTO::PONG::KEYBOARD_OFFLINE);
			data[1] |= _ps2_kbd->getLedsAs(PROTO::PONG::CAPS, PROTO::PONG::SCROLL, PROTO::PONG::NUM);
			data[2] |= PROTO::OUTPUTS::KEYBOARD::PS2;
		}
		if (_usb_mouse_abs) {
			data[1] |= _usb_mouse_abs->getOfflineAs(PROTO::PONG::MOUSE_OFFLINE);
			data[2] |= PROTO::OUTPUTS::MOUSE::USB_ABS;
		} else if (_usb_mouse_rel) {
			data[1] |= _usb_mouse_rel->getOfflineAs(PROTO::PONG::MOUSE_OFFLINE);
			data[2] |= PROTO::OUTPUTS::MOUSE::USB_REL;
		} // TODO: ps2
#		ifdef HID_WITH_USB
		data[3] |= PROTO::FEATURES::HAS_USB;
#		endif
#		ifdef HID_WITH_PS2
		data[3] |= PROTO::FEATURES::HAS_PS2;
#		endif
	} else {
		data[1] = code;
	}
	PROTO::split16(PROTO::crc16(data, 6), &data[6], &data[7]);

#	ifdef CMD_SERIAL
	CMD_SERIAL.write(data, 8);
#	elif defined(CMD_SPI)
	_spiWrite(data);
#	endif
}

int main() {
	init(); // Embedded
	initVariant(); // Arduino
	_initOutputs();

#	ifdef CMD_SERIAL
	CMD_SERIAL.begin(CMD_SERIAL_SPEED);
	unsigned long last = micros();
	uint8_t buffer[8];
	uint8_t index = 0;

	while (true) {
#		ifdef HID_WITH_PS2
		if (_ps2_kbd) {
			_ps2_kbd->periodic();
		}
#		endif
		if (CMD_SERIAL.available() > 0) {
			buffer[index] = (uint8_t)CMD_SERIAL.read();
			if (index == 7) {
				_sendResponse(_handleRequest(buffer));
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
				_sendResponse(PROTO::RESP::TIMEOUT_ERROR);
				index = 0;
			}
		}
	}

#	elif defined(CMD_SPI)
	pinMode(MISO, OUTPUT);
	SPCR = (1 << SPE) | (1 << SPIE); // Slave, SPI En, IRQ En

	while (true) {
#		ifdef HID_WITH_PS2
		if (_ps2_kbd) {
			_ps2_kbd->periodic();
		}
#		endif
		if (_spiReady()) {
			_sendResponse(_handleRequest((const uint8_t *)_spi_in));
		}
	}

#	endif
	return 0;
}
