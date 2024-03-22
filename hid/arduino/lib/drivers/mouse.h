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

#include <stdint.h>

#include "driver.h"


namespace DRIVERS {
	struct Mouse : public Driver {
		using Driver::Driver;
		virtual void begin() {}
		
		/**
		 * Release all keys
		 */
		virtual void clear() {}
		virtual void sendButtons(
			bool left_select, bool left_state,
			bool right_select, bool right_state,
			bool middle_select, bool middle_state,
			bool up_select, bool up_state,
			bool down_select, bool down_state) {}
		virtual void sendMove(int x, int y) {}
		virtual void sendRelative(int x, int y) {}
		virtual void sendWheel(int delta_y) {}
		virtual bool isOffline() { return false; }
		virtual void periodic() {}
	};
}
