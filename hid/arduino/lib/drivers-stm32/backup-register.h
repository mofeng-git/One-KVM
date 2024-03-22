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

# pragma once


#include <stm32f1_rtc.h>

#include "storage.h"


namespace DRIVERS {
	struct BackupRegister : public Storage {
		BackupRegister() : Storage(NON_VOLATILE_STORAGE) {
			_rtc.enableClockInterface();
		}

		void readBlock(void *dest, const void *src, size_t size) override {
			uint8_t *dest_ = reinterpret_cast<uint8_t*>(dest);
			for(size_t index = 0; index < size; ++index) {
				dest_[index] = _rtc.getBackupRegister(reinterpret_cast<uintptr_t>(src) + index + 1);
			}
		}

		void updateBlock(const void *src, void *dest, size_t size) override {
			const uint8_t *src_ = reinterpret_cast<const uint8_t*>(src);
			for(size_t index = 0; index < size; ++index) {
				_rtc.setBackupRegister(reinterpret_cast<uintptr_t>(dest) + index + 1, src_[index]);
			}
		}

		private:
			STM32F1_RTC _rtc;
	};
}
