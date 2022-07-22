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

#pragma once

#include "mouse.h"
#include "hid-wrapper-stm32.h"
#include <USBComposite.h>

namespace DRIVERS {

	const uint8_t reportDescriptionMouseRelative[] = {
		HID_MOUSE_REPORT_DESCRIPTOR()
	};

	class UsbMouseRelative : public Mouse {
		public:
			UsbMouseRelative(HidWrapper& _hidWrapper) : Mouse(USB_MOUSE_RELATIVE),
			_hidWrapper(_hidWrapper), _mouse(_hidWrapper.usbHid) {
				_hidWrapper.addReportDescriptor(reportDescriptionMouseRelative, sizeof(reportDescriptionMouseRelative));
			}

			void begin() override {
				_hidWrapper.begin();
			}

			void clear() override {
				_mouse.release(0xff);
			}

			void sendButtons (
			bool left_select, bool left_state,
			bool right_select, bool right_state,
			bool middle_select, bool middle_state,
			bool up_select, bool up_state,
			bool down_select, bool down_state) override {
				if(left_select) left_state ? _mouse.press(MOUSE_LEFT) : _mouse.release(MOUSE_LEFT);
				if(right_select) right_state ? _mouse.press(MOUSE_RIGHT) : _mouse.release(MOUSE_RIGHT);
				if(middle_select) middle_state ? _mouse.press(MOUSE_MIDDLE) : _mouse.release(MOUSE_MIDDLE);
			}

			void sendRelative(int x, int y) override {
				_mouse.move(x, y);
			}

			void sendWheel(int delta_y) override {
				_mouse.move(0, 0, delta_y);
			}

			bool isOffline() override {
				return USBComposite == false;
			}

		private:
			HidWrapper& _hidWrapper;
			HIDMouse _mouse;
	};
}
