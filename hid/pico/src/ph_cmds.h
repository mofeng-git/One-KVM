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


u8 ph_cmd_kbd_get_leds(void);
u8 ph_cmd_get_offlines(void);

void ph_cmd_set_kbd(const u8 *args);
void ph_cmd_set_mouse(const u8 *args);

void ph_cmd_send_clear(const u8 *args);
void ph_cmd_kbd_send_key(const u8 *args);
void ph_cmd_mouse_send_button(const u8 *args);
void ph_cmd_mouse_send_abs(const u8 *args);
void ph_cmd_mouse_send_rel(const u8 *args);
void ph_cmd_mouse_send_wheel(const u8 *args);
