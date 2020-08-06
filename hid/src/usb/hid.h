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

#include "../inline.h"

#include "keymap.h"


// -----------------------------------------------------------------------------
class UsbHid {
	public:
		UsbHid() {}

		void begin() {
			BootKeyboard.begin();
			SingleAbsoluteMouse.begin();
		}

		void reset() {
			BootKeyboard.releaseAll();
			SingleAbsoluteMouse.releaseAll();
		}

		INLINE void sendKey(uint8_t code, bool state) {
			KeyboardKeycode usb_code = keymapUsb(code);
			if (usb_code != KEY_ERROR_UNDEFINED) {
				if (state) BootKeyboard.press(usb_code);
				else BootKeyboard.release(usb_code);
			}
		}

		INLINE void sendMouseButtons(
			bool left_select, bool left_state,
			bool right_select, bool right_state,
			bool middle_select, bool middle_state
		) {
			if (left_select) sendMouseButton(MOUSE_LEFT, left_state);
			if (right_select) sendMouseButton(MOUSE_RIGHT, right_state);
			if (middle_select) sendMouseButton(MOUSE_MIDDLE, middle_state);
		}

		INLINE void sendMouseMove(int x, int y) {
			SingleAbsoluteMouse.moveTo(x, y);
		}

		INLINE void sendMouseWheel(int delta_y) {
			// delta_x is not supported by hid-project now
			SingleAbsoluteMouse.move(0, 0, delta_y);
		}

		INLINE uint8_t getLedsAs(uint8_t caps, uint8_t scroll, uint8_t num) {
			uint8_t leds = BootKeyboard.getLeds();
			uint8_t result = 0;

			if (leds & LED_CAPS_LOCK) result |= caps;
			if (leds & LED_SCROLL_LOCK) result |= scroll;
			if (leds & LED_NUM_LOCK) result |= num;
			return result;
		}

	private:
		INLINE void sendMouseButton(uint8_t button, bool state) {
			if (state) SingleAbsoluteMouse.press(button);
			else SingleAbsoluteMouse.release(button);
		}
};
