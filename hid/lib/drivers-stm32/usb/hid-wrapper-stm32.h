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

#include <USBComposite.h>

namespace DRIVERS {

	class HidWrapper {
		public:
			HidWrapper(USBCompositeSerial* _serial = nullptr) : _serial(_serial) {}

			void begin() {
				if(_init)
					return;
				_init = true;

				_report_descriptor_length = 0;
				for(uint8 i = 0; i<_count; ++i) {
					_report_descriptor_length += _descriptors_size[i];
				}

				_report_descriptor = new uint8[_report_descriptor_length];

				uint16_t index = 0;
				for(uint8 i = 0; i<_count; ++i) {
					memcpy(_report_descriptor + index, _report_descriptors[i], _descriptors_size[i]);
					index += _descriptors_size[i];
				}

				if(_serial) {
					usbHid.begin(*_serial, _report_descriptor, _report_descriptor_length);
				} else {
					usbHid.begin(_report_descriptor, _report_descriptor_length);
				}
			}
			
			void addReportDescriptor(const uint8_t* report_descriptor, uint16_t report_descriptor_length) {
				_report_descriptors[_count] = report_descriptor;
				_descriptors_size[_count] = report_descriptor_length;
				++_count;
			}

			USBCompositeSerial* serial() {
				return _serial;
			}

			USBHID usbHid;
		
		private:
			USBCompositeSerial* _serial;
			bool _init = false;

			static constexpr uint8_t MAX_USB_DESCRIPTORS = 2;
			const uint8_t* _report_descriptors[MAX_USB_DESCRIPTORS];
			uint8_t _descriptors_size[MAX_USB_DESCRIPTORS];

			uint8_t _count = 0;
			uint8_t* _report_descriptor;
			uint16_t _report_descriptor_length;
	};
}
