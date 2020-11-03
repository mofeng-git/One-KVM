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

#include <HID-Project.h>

#include "keymap.h"


// -----------------------------------------------------------------------------
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

		void sendMouseButtons(
			bool left_select, bool left_state,
			bool right_select, bool right_state,
			bool middle_select, bool middle_state,
			bool up_select, bool up_state,
			bool down_select, bool down_state
		) {
			if (left_select) _sendMouseButton(MOUSE_LEFT, left_state);
			if (right_select) _sendMouseButton(MOUSE_RIGHT, right_state);
			if (middle_select) _sendMouseButton(MOUSE_MIDDLE, middle_state);
			if (up_select) _sendMouseButton(MOUSE_PREV, up_state);
			if (down_select) _sendMouseButton(MOUSE_NEXT, down_state);
		}

		void sendMouseMove(int x, int y) {
			SingleAbsoluteMouse.moveTo(x, y);
		}

		void sendMouseWheel(int delta_y) {
			// delta_x is not supported by hid-project now
			SingleAbsoluteMouse.move(0, 0, delta_y);
		}

	private:
		void _sendMouseButton(uint8_t button, bool state) {
			if (state) SingleAbsoluteMouse.press(button);
			else SingleAbsoluteMouse.release(button);
		}
};
