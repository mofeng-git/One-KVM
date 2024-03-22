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

#include "board.h"
#include <libmaple/iwdg.h>


namespace DRIVERS {
	class BoardStm32 : public Board {
		public:
			BoardStm32() : Board(BOARD){
				//2 sec timeout
				iwdg_init(IWDG_PRE_16, 0xFFF);
				pinMode(LED_BUILTIN, OUTPUT);
			}

			void reset() override {
				nvic_sys_reset();
			}

			void periodic() override {
				iwdg_feed();
				if (is_micros_timed_out(_prev_ts, 100000)) {
					switch(_state) {
						case 0:
							digitalWrite(LED_BUILTIN, LOW);
							break;
						case 2:
							if(_rx_data) {
								_rx_data = false;
								digitalWrite(LED_BUILTIN, LOW);
							}
							break;
						case 4:
							if(_keyboard_online) {
								_keyboard_online = false;
								digitalWrite(LED_BUILTIN, LOW);
							}
							break;
						case 8:
							if(_mouse_online) {
								_mouse_online = false;
								digitalWrite(LED_BUILTIN, LOW);
							}
							break;
						case 1:	// heartbeat off
						case 3:	// _rx_data off
						case 7: // _keyboard_online off
						case 11: // _mouse_online off
							digitalWrite(LED_BUILTIN, HIGH);
							break;
						case 19:
							_state = -1;
							break;
					}
					++_state;
					_prev_ts = micros();
				}
			}

			void updateStatus(status status) override {
				switch (status) {
					case RX_DATA:
						_rx_data = true;
						break;
					case KEYBOARD_ONLINE:
						_keyboard_online = true;
						break;
					case MOUSE_ONLINE:
						_mouse_online = true;
						break;
				}
			}

		private:
			unsigned long _prev_ts = 0;
			uint8_t _state = 0;
			bool _rx_data = false;
			bool _keyboard_online = false;
			bool _mouse_online = false;
	};
}
