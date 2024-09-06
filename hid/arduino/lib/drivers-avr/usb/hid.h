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

#include <Arduino.h>
#include <HID-Project.h>

#include "keyboard.h"
#include "mouse.h"
#include "tools.h"
#include "usb-keymap.h"
#ifdef AUM
#	include "aum.h"
#endif

using namespace DRIVERS;

// -----------------------------------------------------------------------------
#ifdef HID_USB_CHECK_ENDPOINT
// https://github.com/arduino/ArduinoCore-avr/blob/2f67c916f6ab6193c404eebe22efe901e0f9542d/cores/arduino/USBCore.cpp#L249
// https://sourceforge.net/p/arduinomidilib/svn/41/tree/branch/3.1/Teensy/teensy_core/usb_midi/usb_api.cpp#l103
#	ifdef AUM
#		define CHECK_AUM_USB { if (!aumIsUsbConnected()) { return true; } }
#	else
#		define CHECK_AUM_USB
#	endif
#	define CLS_IS_OFFLINE(_hid) \
		bool isOffline() override { \
			CHECK_AUM_USB; \
			uint8_t ep = _hid.getPluggedEndpoint(); \
			uint8_t intr_state = SREG; \
			cli(); \
			UENUM = ep & 7; \
			bool rw_allowed = UEINTX & (1 << RWAL); \
			SREG = intr_state; \
			if (rw_allowed) { \
				return false; \
			} \
			return true; \
		}
#	define CHECK_HID_EP { if (isOffline()) return; }

#else
#	define CLS_IS_OFFLINE(_hid) \
		bool isOffline() override { \
			return false; \
		}
#	define CHECK_HID_EP

#endif


class UsbKeyboard : public DRIVERS::Keyboard {
	public:
		UsbKeyboard() : DRIVERS::Keyboard(DRIVERS::USB_KEYBOARD) {}

		void begin() override {
			_kbd.begin();
		}

		void periodic() override {
#			ifdef HID_USB_CHECK_ENDPOINT
			static unsigned long prev_ts = 0;
			if (is_micros_timed_out(prev_ts, 50000)) {
				static bool prev_online = true;
				bool online = !isOffline();
				if (!_sent || (online && !prev_online)) {
					_sendCurrent();
				}
				prev_online = online;
				prev_ts = micros();
			}
#			endif
		}

		void clear() override {
			_kbd.releaseAll();
		}

		void sendKey(uint8_t code, bool state) override {
			enum KeyboardKeycode usb_code = keymapUsb(code);
			if (usb_code > 0) {
				if (state ? _kbd.add(usb_code) : _kbd.remove(usb_code)) {
					_sendCurrent();
				}
			}
		}

		CLS_IS_OFFLINE(_kbd)

		KeyboardLedsState getLeds() override {
			uint8_t leds = _kbd.getLeds();
			KeyboardLedsState result = {
				.caps = leds & LED_CAPS_LOCK,
				.scroll = leds & LED_SCROLL_LOCK,
				.num = leds & LED_NUM_LOCK,
			};
			return result;
		}

	private:
		BootKeyboard_ _kbd;
		bool _sent = true;

		void _sendCurrent() {
#			ifdef HID_USB_CHECK_ENDPOINT
			if (isOffline()) {
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
		) override { \
			if (left_select) _sendButton(MOUSE_LEFT, left_state); \
			if (right_select) _sendButton(MOUSE_RIGHT, right_state); \
			if (middle_select) _sendButton(MOUSE_MIDDLE, middle_state); \
			if (up_select) _sendButton(MOUSE_PREV, up_state); \
			if (down_select) _sendButton(MOUSE_NEXT, down_state); \
		}

class UsbMouseAbsolute : public DRIVERS::Mouse {
	public:
		UsbMouseAbsolute(DRIVERS::type _type) : Mouse(_type) {}

		void begin() override {
			_mouse.begin();
			_mouse.setWin98FixEnabled(getType() == DRIVERS::USB_MOUSE_ABSOLUTE_WIN98);
		}

		void clear() override {
			_mouse.releaseAll();
		}

		CLS_SEND_BUTTONS

		void sendMove(int x, int y) override {
			CHECK_HID_EP;
			_mouse.moveTo(x, y);
		}

		void sendWheel(int delta_y) override {
			// delta_x is not supported by hid-project now
			CHECK_HID_EP;
			_mouse.move(0, 0, delta_y);
		}

		CLS_IS_OFFLINE(_mouse)

	private:
		SingleAbsoluteMouse_ _mouse;

		void _sendButton(uint8_t button, bool state) {
			CHECK_HID_EP;
			if (state) _mouse.press(button);
			else _mouse.release(button);
		}
};

class UsbMouseRelative : public DRIVERS::Mouse {
	public:
		UsbMouseRelative() : DRIVERS::Mouse(DRIVERS::USB_MOUSE_RELATIVE) {}

		void begin() override {
			_mouse.begin();
		}

		void clear() override {
			_mouse.releaseAll();
		}

		CLS_SEND_BUTTONS

		void sendRelative(int x, int y) override {
			CHECK_HID_EP;
			_mouse.move(x, y, 0);
		}

		void sendWheel(int delta_y) override {
			// delta_x is not supported by hid-project now
			CHECK_HID_EP;
			_mouse.move(0, 0, delta_y);
		}

		CLS_IS_OFFLINE(_mouse)

	private:
		BootMouse_ _mouse;

		void _sendButton(uint8_t button, bool state) {
			CHECK_HID_EP;
			if (state) _mouse.press(button);
			else _mouse.release(button);
		}
};

#undef CLS_SEND_BUTTONS
#undef CLS_IS_OFFLINE
#undef CHECK_HID_EP
