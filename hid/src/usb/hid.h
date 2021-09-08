/*****************************************************************************
#                                                                            #
#    KVMD - The main PiKVM daemon.                                           #
#                                                                            #
#    Copyright (C) 2018-2021  Maxim Devaev <mdevaev@gmail.com>               #
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

#include <Arduino.h>
#include <HID-Project.h>

#include "../tools.h"
#ifdef AUM
#	include "../aum.h"
#endif
#include "keymap.h"


// -----------------------------------------------------------------------------
#ifdef HID_USB_CHECK_ENDPOINT
// https://github.com/arduino/ArduinoCore-avr/blob/2f67c916f6ab6193c404eebe22efe901e0f9542d/cores/arduino/USBCore.cpp#L249
// https://sourceforge.net/p/arduinomidilib/svn/41/tree/branch/3.1/Teensy/teensy_core/usb_midi/usb_api.cpp#l103
#	ifdef AUM
#		define CHECK_AUM_USB { if (!aumIsUsbConnected()) { return offline; } }
#	else
#		define CHECK_AUM_USB
#	endif
#	define CLS_GET_OFFLINE_AS(_hid) \
		uint8_t getOfflineAs(uint8_t offline) { \
			CHECK_AUM_USB; \
			uint8_t ep = _hid.getPluggedEndpoint(); \
			uint8_t intr_state = SREG; \
			cli(); \
			UENUM = ep & 7; \
			bool rw_allowed = UEINTX & (1 << RWAL); \
			SREG = intr_state; \
			if (rw_allowed) { \
				return 0; \
			} \
			return offline; \
		}
#	define CHECK_HID_EP { if (getOfflineAs(1)) return; }

#else
#	define CLS_GET_OFFLINE_AS(_hid) \
		uint8_t getOfflineAs(uint8_t offline) { \
			return 0; \
		}
#	define CHECK_HID_EP

#endif

class UsbKeyboard {
	public:
		UsbKeyboard() {}

		void begin() {
			_kbd.begin();
		}

		void periodic() {
#			ifdef HID_USB_CHECK_ENDPOINT
			static unsigned long prev_ts = 0;
			if (is_micros_timed_out(prev_ts, 50000)) {
				static bool prev_online = true;
				bool online = !getOfflineAs(1);
				if (!_sent || (online && !prev_online)) {
					_sendCurrent();
				}
				prev_online = online;
				prev_ts = micros();
			}
#			endif
		}

		void clear() {
			_kbd.releaseAll();
		}

		void sendKey(uint8_t code, bool state) {
			KeyboardKeycode usb_code = keymapUsb(code);
			if (usb_code != KEY_ERROR_UNDEFINED) {
				if (state ? _kbd.add(usb_code) : _kbd.remove(usb_code)) {
					_sendCurrent();
				}
			}
		}

		CLS_GET_OFFLINE_AS(_kbd)

		uint8_t getLedsAs(uint8_t caps, uint8_t scroll, uint8_t num) {
			uint8_t leds = _kbd.getLeds();
			uint8_t result = 0;
			if (leds & LED_CAPS_LOCK) result |= caps;
			if (leds & LED_SCROLL_LOCK) result |= scroll;
			if (leds & LED_NUM_LOCK) result |= num;
			return result;
		}

	private:
		BootKeyboard_ _kbd;
		bool _sent = true;

		void _sendCurrent() {
#			ifdef HID_USB_CHECK_ENDPOINT
			if (getOfflineAs(1)) {
				_sent = false;
			} else {
#			endif
				_sent = (_kbd.send() >= 0);
#			ifdef HID_USB_CHECK_ENDPOINT
			}
#			endif
		}
};

#define CLS_SEND_BUTTONS \
		void sendButtons( \
			bool left_select, bool left_state, \
			bool right_select, bool right_state, \
			bool middle_select, bool middle_state, \
			bool up_select, bool up_state, \
			bool down_select, bool down_state \
		) { \
			if (left_select) _sendButton(MOUSE_LEFT, left_state); \
			if (right_select) _sendButton(MOUSE_RIGHT, right_state); \
			if (middle_select) _sendButton(MOUSE_MIDDLE, middle_state); \
			if (up_select) _sendButton(MOUSE_PREV, up_state); \
			if (down_select) _sendButton(MOUSE_NEXT, down_state); \
		}

class UsbMouseAbsolute {
	public:
		UsbMouseAbsolute() {}

		void begin(bool win98_fix) {
			_mouse.begin();
			_mouse.setWin98FixEnabled(win98_fix);
		}

		bool isWin98FixEnabled() {
			return _mouse.isWin98FixEnabled();
		}

		void clear() {
			_mouse.releaseAll();
		}

		CLS_SEND_BUTTONS

		void sendMove(int x, int y) {
			CHECK_HID_EP;
			_mouse.moveTo(x, y);
		}

		void sendWheel(int delta_y) {
			// delta_x is not supported by hid-project now
			CHECK_HID_EP;
			_mouse.move(0, 0, delta_y);
		}

		CLS_GET_OFFLINE_AS(_mouse)

	private:
		SingleAbsoluteMouse_ _mouse;

		void _sendButton(uint8_t button, bool state) {
			CHECK_HID_EP;
			if (state) _mouse.press(button);
			else _mouse.release(button);
		}
};

class UsbMouseRelative {
	public:
		UsbMouseRelative() {}

		void begin() {
			_mouse.begin();
		}

		void clear() {
			_mouse.releaseAll();
		}

		CLS_SEND_BUTTONS

		void sendRelative(int x, int y) {
			CHECK_HID_EP;
			_mouse.move(x, y, 0);
		}

		void sendWheel(int delta_y) {
			// delta_x is not supported by hid-project now
			CHECK_HID_EP;
			_mouse.move(0, 0, delta_y);
		}

		CLS_GET_OFFLINE_AS(_mouse)

	private:
		BootMouse_ _mouse;

		void _sendButton(uint8_t button, bool state) {
			CHECK_HID_EP;
			if (state) _mouse.press(button);
			else _mouse.release(button);
		}
};

#undef CLS_SEND_BUTTONS
#undef CLS_GET_OFFLINE_AS
#undef CHECK_HID_EP
