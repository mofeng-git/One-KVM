/* ========================================================================= #
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
# ========================================================================= */


#pragma once

#include "ph_types.h"
#include "ph_proto.h"


#define PH_O_HAS_PS2			(!!(ph_g_outputs_avail & PH_PROTO_OUT2_HAS_PS2))
#define PH_O_KBD(x_id)			((ph_g_outputs_active & PH_PROTO_OUT1_KBD_MASK) == PH_PROTO_OUT1_KBD_##x_id)
#define PH_O_MOUSE(x_id)		((ph_g_outputs_active & PH_PROTO_OUT1_MOUSE_MASK) == PH_PROTO_OUT1_MOUSE_##x_id)
#define PH_O_IS_KBD_USB			PH_O_KBD(USB)
#define PH_O_IS_MOUSE_USB		(PH_O_MOUSE(USB_ABS) || PH_O_MOUSE(USB_REL) || PH_O_MOUSE(USB_W98))
#define PH_O_IS_MOUSE_USB_ABS	(PH_O_MOUSE(USB_ABS) || PH_O_MOUSE(USB_W98))
#define PH_O_IS_MOUSE_USB_REL	PH_O_MOUSE(USB_REL)
#define PH_O_IS_KBD_PS2			PH_O_KBD(PS2)
#define PH_O_IS_MOUSE_PS2		PH_O_MOUSE(PS2)


extern bool ph_g_is_bridge;
extern u8 ph_g_outputs_active;
extern u8 ph_g_outputs_avail;


void ph_outputs_init(void);
void ph_outputs_write(u8 mask, u8 outputs, bool force);
