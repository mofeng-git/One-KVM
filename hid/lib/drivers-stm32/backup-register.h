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

#include "storage.h"
#include <stm32f1_rtc.h>

namespace DRIVERS {
	struct BackupRegister : public Storage {
		BackupRegister() : Storage(NON_VOLATILE_STORAGE) {
			_rtc.enableClockInterface();
		}

		void readBlock(void *dest, const void *src, size_t size) override {
			uint8_t* _dest = reinterpret_cast<uint8_t*>(dest);
			for(size_t i = 0; i < size; ++i) {
				_dest[i] = _rtc.getBackupRegister(reinterpret_cast<uintptr_t>(src) + i + 1);
			}
		}

		void updateBlock(const void *src, void *dest, size_t size) override {
			const uint8_t*  _src = reinterpret_cast<const uint8_t*>(src);
			for(size_t i = 0; i < size; ++i) {
				_rtc.setBackupRegister(reinterpret_cast<uintptr_t>(dest) + i + 1, _src[i]);
			}
		}

		private:
			STM32F1_RTC _rtc;
	};
}
