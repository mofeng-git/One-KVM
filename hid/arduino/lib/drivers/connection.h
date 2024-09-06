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

#include "driver.h"
#include "stdint.h"


namespace DRIVERS {
	typedef void (*DataHandler)(const uint8_t *data, size_t size);
	typedef void (*TimeoutHandler)();

	struct Connection : public Driver {
		using Driver::Driver;

		virtual void begin() {}
		
		virtual void periodic() {}

		void onTimeout(TimeoutHandler cb) {
			_timeout_cb = cb;
		}

		void onData(DataHandler cb) {
			_data_cb = cb;
		}

		virtual void write(const uint8_t *data, size_t size) = 0;
		
		protected:
			TimeoutHandler _timeout_cb = nullptr;
			DataHandler _data_cb = nullptr;
	};
}
