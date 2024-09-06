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

#include <USBComposite.h>

#include "mouse.h"
#include "hid-wrapper-stm32.h"


namespace DRIVERS {
	const uint8_t reportDescriptionMouseAbsolute[] = {
		HID_ABS_MOUSE_REPORT_DESCRIPTOR()
	};

	class UsbMouseAbsolute : public Mouse {
		public:
			UsbMouseAbsolute(HidWrapper& _hidWrapper) : Mouse(USB_MOUSE_ABSOLUTE),
			_hidWrapper(_hidWrapper), _mouse(_hidWrapper.usbHid) {
				_hidWrapper.addReportDescriptor(reportDescriptionMouseAbsolute, sizeof(reportDescriptionMouseAbsolute));
			}

			void begin() override {
				_hidWrapper.begin();
			}

			void clear() override {
				_mouse.release(0xFF);
			}

			void sendButtons (
				bool left_select, bool left_state,
				bool right_select, bool right_state,
				bool middle_select, bool middle_state,
				bool up_select, bool up_state,
				bool down_select, bool down_state) override {

#				define SEND_BUTTON(x_low, x_up) { \
						if (x_low##_select) { \
							if (x_low##_state) _mouse.press(MOUSE_##x_up); \
							else _mouse.release(MOUSE_##x_up); \
						} \
					}
				SEND_BUTTON(left, LEFT);
				SEND_BUTTON(right, RIGHT);
				SEND_BUTTON(middle, MIDDLE);
#				undef SEND_BUTTON
			}

			void sendMove(int x, int y) override {
				_mouse.move(x, y);
			}

			void sendWheel(int delta_y) override {
				_mouse.move(0, 0, delta_y);
			}

			bool isOffline() override {
				return (USBComposite == false);
			}

		private:
			HidWrapper& _hidWrapper;
			HIDAbsMouse _mouse;
	};
}
