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
#    but WITHOUT ANY WARRANTY without even the implied warranty of          #
#    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the           #
#    GNU General Public License for more details.                            #
#                                                                            #
#    You should have received a copy of the GNU General Public License       #
#    along with this program.  If not, see <https://www.gnu.org/licenses/>.  #
#                                                                            #
*****************************************************************************/


#pragma once

#include "ph_types.h"


#define PH_PROTO_MAGIC					((u8)0x33)
#define PH_PROTO_MAGIC_RESP				((u8)0x34)

//#define PH_PROTO_RESP_OK				((u8)0x20) // Legacy
#define PH_PROTO_RESP_NONE				((u8)0x24)
#define PH_PROTO_RESP_CRC_ERROR			((u8)0x40)
#define PH_PROTO_RESP_INVALID_ERROR		((u8)0x45)
#define PH_PROTO_RESP_TIMEOUT_ERROR		((u8)0x48)

// Complex response flags
#define PH_PROTO_PONG_OK				((u8)0b10000000)
#define PH_PROTO_PONG_CAPS				((u8)0b00000001)
#define PH_PROTO_PONG_SCROLL			((u8)0b00000010)
#define PH_PROTO_PONG_NUM				((u8)0b00000100)
#define PH_PROTO_PONG_KBD_OFFLINE		((u8)0b00001000)
#define PH_PROTO_PONG_MOUSE_OFFLINE		((u8)0b00010000)
#define PH_PROTO_PONG_RESET_REQUIRED	((u8)0b01000000)

// Complex request/response flags
#define PH_PROTO_OUT1_DYNAMIC			((u8)0b10000000)
#define PH_PROTO_OUT1_KBD_MASK			((u8)0b00000111)
#define PH_PROTO_OUT1_KBD_USB			((u8)0b00000001)
#define PH_PROTO_OUT1_KBD_PS2			((u8)0b00000011)
// +
#define PH_PROTO_OUT1_MOUSE_MASK		((u8)0b00111000)
#define PH_PROTO_OUT1_MOUSE_USB_ABS		((u8)0b00001000)
#define PH_PROTO_OUT1_MOUSE_USB_REL		((u8)0b00010000)
#define PH_PROTO_OUT1_MOUSE_PS2			((u8)0b00011000)
#define PH_PROTO_OUT1_MOUSE_USB_W98		((u8)0b00100000)

// Complex response
#define PH_PROTO_OUT2_CONNECTABLE		((u8)0b10000000)
#define PH_PROTO_OUT2_CONNECTED			((u8)0b01000000)
#define PH_PROTO_OUT2_HAS_USB			((u8)0b00000001)
#define PH_PROTO_OUT2_HAS_PS2			((u8)0b00000010)
#define PH_PROTO_OUT2_HAS_USB_W98		((u8)0b00000100)

#define PH_PROTO_CMD_PING				((u8)0x01)
#define PH_PROTO_CMD_REPEAT				((u8)0x02)
#define PH_PROTO_CMD_SET_KBD			((u8)0x03)
#define PH_PROTO_CMD_SET_MOUSE			((u8)0x04)
#define PH_PROTO_CMD_SET_CONNECTED		((u8)0x05)
#define PH_PROTO_CMD_CLEAR_HID			((u8)0x10)
// +
#define PH_PROTO_CMD_KBD_KEY			((u8)0x11)
// +
#define PH_PROTO_CMD_MOUSE_ABS			((u8)0x12)
#define PH_PROTO_CMD_MOUSE_BUTTON		((u8)0x13)
#define PH_PROTO_CMD_MOUSE_WHEEL		((u8)0x14)
#define PH_PROTO_CMD_MOUSE_REL			((u8)0x15)
// +
#define PH_PROTO_CMD_MOUSE_LEFT_SELECT		((u8)0b10000000)
#define PH_PROTO_CMD_MOUSE_LEFT_STATE		((u8)0b00001000)
// +
#define PH_PROTO_CMD_MOUSE_RIGHT_SELECT		((u8)0b01000000)
#define PH_PROTO_CMD_MOUSE_RIGHT_STATE		((u8)0b00000100)
// +
#define PH_PROTO_CMD_MOUSE_MIDDLE_SELECT	((u8)0b00100000)
#define PH_PROTO_CMD_MOUSE_MIDDLE_STATE		((u8)0b00000010)
// +
#define PH_PROTO_CMD_MOUSE_BACKWARD_SELECT	((u8)0b10000000) // Previous/Up
#define PH_PROTO_CMD_MOUSE_BACKWARD_STATE	((u8)0b00001000) // Previous/Up
// +
#define PH_PROTO_CMD_MOUSE_FORWARD_SELECT	((u8)0b01000000) // Next/Down
#define PH_PROTO_CMD_MOUSE_FORWARD_STATE	((u8)0b00000100) // Next/Down
