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


namespace DRIVERS {
	class HidWrapper {
		public:
			void begin() {
				if (_init) {
					return;
				}
				_init = true;

				_report_descriptor_length = 0;
				for (unsigned index = 0; index < _count; ++index) {
					_report_descriptor_length += _descriptors_size[index];
				}

				_report_descriptor = new uint8[_report_descriptor_length];

				size_t offset = 0;
				for (unsigned index = 0; index < _count; ++index) {
					memcpy(_report_descriptor + offset, _report_descriptors[index], _descriptors_size[index]);
					offset += _descriptors_size[index];
				}

				usbHid.begin(_report_descriptor, _report_descriptor_length);
			}
			
			void addReportDescriptor(const uint8_t *report_descriptor, uint16_t report_descriptor_length) {
				_report_descriptors[_count] = report_descriptor;
				_descriptors_size[_count] = report_descriptor_length;
				++_count;
			}

			USBHID usbHid;
		
		private:
			bool _init = false;

			static constexpr uint8_t MAX_USB_DESCRIPTORS = 2;
			const uint8_t *_report_descriptors[MAX_USB_DESCRIPTORS];
			uint8_t _descriptors_size[MAX_USB_DESCRIPTORS];

			uint8_t _count = 0;
			uint8_t *_report_descriptor;
			uint16_t _report_descriptor_length;
	};
}
