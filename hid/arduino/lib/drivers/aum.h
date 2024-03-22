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

#include <digitalWriteFast.h>


inline void aumInit() {
	pinModeFast(AUM_IS_USB_POWERED_PIN, INPUT);
	pinModeFast(AUM_SET_USB_VBUS_PIN, OUTPUT);
	pinModeFast(AUM_SET_USB_CONNECTED_PIN, OUTPUT);
	digitalWriteFast(AUM_SET_USB_CONNECTED_PIN, HIGH);
}

inline void aumProxyUsbVbus() {
	bool vbus = digitalReadFast(AUM_IS_USB_POWERED_PIN);
	if (digitalReadFast(AUM_SET_USB_VBUS_PIN) != vbus) {
		digitalWriteFast(AUM_SET_USB_VBUS_PIN, vbus);
	}
}

inline void aumSetUsbConnected(bool connected) {
	digitalWriteFast(AUM_SET_USB_CONNECTED_PIN, connected);
}

inline bool aumIsUsbConnected() {
	return digitalReadFast(AUM_SET_USB_CONNECTED_PIN);
}
