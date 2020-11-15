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

#include <Arduino.h>
#include <HID-Project.h>

#include "keymap.h"


// -----------------------------------------------------------------------------
#ifdef CHECK_ENDPOINT
static bool _checkEndpoint(uint8_t ep) {
	// https://github.com/arduino/ArduinoCore-avr/blob/2f67c916f6ab6193c404eebe22efe901e0f9542d/cores/arduino/USBCore.cpp#L249
	// https://sourceforge.net/p/arduinomidilib/svn/41/tree/branch/3.1/Teensy/teensy_core/usb_midi/usb_api.cpp#l103
	uint8_t intr_state = SREG;
	cli();
	UENUM = ep & 7;
	bool rw_allowed = UEINTX & (1 << RWAL);
	SREG = intr_state;
	return rw_allowed;
}
#	define CHECK_HID_EP(_hid) { if (!_checkEndpoint(_hid.getPluggedEndpoint())) return; }
#else
#	define CHECK_HID_EP(_hid)
#endif

class UsbHidKeyboard {
	public:
		UsbHidKeyboard() {}

		void begin() {
			BootKeyboard.begin();
		}

		void reset() {
			BootKeyboard.releaseAll();
		}

		void sendKey(uint8_t code, bool state) {
			CHECK_HID_EP(BootKeyboard);
			KeyboardKeycode usb_code = keymapUsb(code);
			if (usb_code != KEY_ERROR_UNDEFINED) {
				if (state) BootKeyboard.press(usb_code);
				else BootKeyboard.release(usb_code);
			}
		}

		uint8_t getLedsAs(uint8_t caps, uint8_t scroll, uint8_t num) {
			uint8_t leds = BootKeyboard.getLeds();
			uint8_t result = 0;

			if (leds & LED_CAPS_LOCK) result |= caps;
			if (leds & LED_SCROLL_LOCK) result |= scroll;
			if (leds & LED_NUM_LOCK) result |= num;
			return result;
		}
};

class UsbHidMouse {
	public:
		UsbHidMouse() {}

		void begin() {
			SingleAbsoluteMouse.begin();
		}

		void reset() {
			SingleAbsoluteMouse.releaseAll();
		}

		void sendButtons(
			bool left_select, bool left_state,
			bool right_select, bool right_state,
			bool middle_select, bool middle_state,
			bool up_select, bool up_state,
			bool down_select, bool down_state
		) {
			if (left_select) _sendButton(MOUSE_LEFT, left_state);
			if (right_select) _sendButton(MOUSE_RIGHT, right_state);
			if (middle_select) _sendButton(MOUSE_MIDDLE, middle_state);
			if (up_select) _sendButton(MOUSE_PREV, up_state);
			if (down_select) _sendButton(MOUSE_NEXT, down_state);
		}

		void sendMove(int x, int y) {
			CHECK_HID_EP(SingleAbsoluteMouse);
			SingleAbsoluteMouse.moveTo(x, y);
		}

		void sendWheel(int delta_y) {
			CHECK_HID_EP(SingleAbsoluteMouse);
			// delta_x is not supported by hid-project now
			SingleAbsoluteMouse.move(0, 0, delta_y);
		}

	private:
		void _sendButton(uint8_t button, bool state) {
			CHECK_HID_EP(SingleAbsoluteMouse);
			if (state) SingleAbsoluteMouse.press(button);
			else SingleAbsoluteMouse.release(button);
		}
};

#undef CHECK_HID_EP
