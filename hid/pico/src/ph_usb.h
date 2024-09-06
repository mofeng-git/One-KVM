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


extern u8 ph_g_usb_kbd_leds;
extern bool ph_g_usb_kbd_online;
extern bool ph_g_usb_mouse_online;


void ph_usb_init(void);
void ph_usb_task(void);

void ph_usb_kbd_send_key(u8 key, bool state);

void ph_usb_mouse_send_button(u8 button, bool state);
void ph_usb_mouse_send_abs(s16 x, s16 y);
void ph_usb_mouse_send_rel(s8 x, s8 y);
void ph_usb_mouse_send_wheel(s8 h, s8 v);

void ph_usb_send_clear(void);
