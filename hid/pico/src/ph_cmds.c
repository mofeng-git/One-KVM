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


#include "tusb.h"

#include "ph_types.h"
#include "ph_tools.h"
#include "ph_proto.h"
#include "ph_outputs.h"
#include "ph_usb.h"
#include "ph_usb_keymap.h"
#include "ph_ps2.h"


u8 ph_cmd_kbd_get_leds(void) {
	u8 leds = 0;
	if (PH_O_IS_KBD_USB) {
		leds = ph_g_usb_kbd_leds;
	} else if (PH_O_IS_KBD_PS2) {
		leds = ph_g_ps2_kbd_leds;
	}
#	define GET(x_mod) ((leds & KEYBOARD_LED_##x_mod##LOCK) ? PH_PROTO_PONG_##x_mod : 0)
	return (GET(CAPS) | GET(SCROLL) | GET(NUM));
#	undef GET
}

u8 ph_cmd_get_offlines(void) {
	bool kbd_online = true;
	if (PH_O_IS_KBD_USB) {
		kbd_online = ph_g_usb_kbd_online;
	} else if (PH_O_IS_KBD_PS2) {
		kbd_online = ph_g_ps2_kbd_online;
	}
	bool mouse_online = true;
	if (PH_O_IS_MOUSE_USB) {
		mouse_online = ph_g_usb_mouse_online;
	} else if (PH_O_IS_MOUSE_PS2) {
		mouse_online = ph_g_ps2_mouse_online;
	}
	return (
		(kbd_online ? 0 : PH_PROTO_PONG_KBD_OFFLINE)
		| (mouse_online ? 0 : PH_PROTO_PONG_MOUSE_OFFLINE)
	);
}

void ph_cmd_set_kbd(const u8 *args) { // 1 byte
	ph_outputs_write(PH_PROTO_OUT1_KBD_MASK, args[0], false);
}

void ph_cmd_set_mouse(const u8 *args) { // 1 byte
	ph_outputs_write(PH_PROTO_OUT1_MOUSE_MASK, args[0], false);
}

void ph_cmd_send_clear(const u8 *args) { // 0 bytes
	(void)args;
	ph_usb_send_clear();
	ph_ps2_send_clear();
}

void ph_cmd_kbd_send_key(const u8 *args) { // 2 bytes
	const u8 key = ph_usb_keymap(args[0]);
	if (key > 0) {
		if (PH_O_IS_KBD_USB) {
			ph_usb_kbd_send_key(key, args[1]);
		} else if (PH_O_IS_KBD_PS2) {
			ph_ps2_kbd_send_key(key, args[1]);
		}
	}
}

void ph_cmd_mouse_send_button(const u8 *args) { // 2 bytes
#	define HANDLE(x_byte_n, x_button) { \
			if (args[x_byte_n] & PH_PROTO_CMD_MOUSE_##x_button##_SELECT) { \
				const bool m_state = !!(args[x_byte_n] & PH_PROTO_CMD_MOUSE_##x_button##_STATE); \
				if (PH_O_IS_MOUSE_USB) { \
					ph_usb_mouse_send_button(MOUSE_BUTTON_##x_button, m_state); \
				} else if (PH_O_IS_MOUSE_PS2) { \
					ph_ps2_mouse_send_button(MOUSE_BUTTON_##x_button, m_state); \
				} \
			} \
		}
	HANDLE(0, LEFT);
	HANDLE(0, RIGHT);
	HANDLE(0, MIDDLE);
	HANDLE(1, BACKWARD);
	HANDLE(1, FORWARD);
#	undef HANDLE
}

void ph_cmd_mouse_send_abs(const u8 *args) { // 4 bytes
	if (PH_O_IS_MOUSE_USB_ABS) {
		const s16 x = ph_merge8_s16(args[0], args[1]);
		const s16 y = ph_merge8_s16(args[2], args[3]);
		ph_usb_mouse_send_abs(x, y);
	}
}

void ph_cmd_mouse_send_rel(const u8 *args) { // 2 bytes
	if (PH_O_IS_MOUSE_USB_REL) {
		ph_usb_mouse_send_rel(args[0], args[1]);
	} else if (PH_O_IS_MOUSE_PS2) {
		ph_ps2_mouse_send_rel(args[0], args[1]);
	}
}

void ph_cmd_mouse_send_wheel(const u8 *args) { // 2 bytes
	if (PH_O_IS_MOUSE_USB) {
		ph_usb_mouse_send_wheel(args[0], args[1]);
	} else if (PH_O_IS_MOUSE_PS2) {
		ph_ps2_mouse_send_wheel(args[0], args[1]);
	}
}
