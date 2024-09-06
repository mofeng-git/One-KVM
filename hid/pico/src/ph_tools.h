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

#include "ph_types.h"


inline u16 ph_crc16(const u8 *buf, uz len) {
	const u16 polinom = 0xA001;
	u16 crc = 0xFFFF;

	for (uz byte_count = 0; byte_count < len; ++byte_count) {
		crc = crc ^ buf[byte_count];
		for (uz bit_count = 0; bit_count < 8; ++bit_count) {
			if ((crc & 0x0001) == 0) {
				crc = crc >> 1;
			} else {
				crc = crc >> 1;
				crc = crc ^ polinom;
			}
		}
	}
	return crc;
}

inline s16 ph_merge8_s16(u8 a, u8 b) {
	return (((int)a << 8) | (int)b);
}

inline u16 ph_merge8_u16(u8 a, u8 b) {
	return (((u16)a << 8) | (u16)b);
}

inline void ph_split16(u16 from, u8 *to_a, u8 *to_b) {
	*to_a = (u8)(from >> 8);
	*to_b = (u8)(from & 0xFF);
}
