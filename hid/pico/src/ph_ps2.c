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


#include "ph_ps2.h"

#include "ph_types.h"
#include "ph_outputs.h"

#include "hardware/gpio.h"


#define _LS_POWER_PIN	13
#define _KBD_DATA_PIN	11 // CLK == 12
#define _MOUSE_DATA_PIN	14 // CLK == 15


u8 ph_g_ps2_kbd_leds = 0;
bool ph_g_ps2_kbd_online = 0;
bool ph_g_ps2_mouse_online = 0;

u8 ph_ps2_kbd_modifiers = 0;
u8 ph_ps2_mouse_buttons = 0;

void tuh_kb_set_leds(u8 leds) {
	ph_g_ps2_kbd_leds = leds;
}

void ph_ps2_init(void) {
	if (PH_O_HAS_PS2) {
		gpio_init(_LS_POWER_PIN);
		gpio_set_dir(_LS_POWER_PIN, GPIO_OUT);
		gpio_put(_LS_POWER_PIN, true);
	}

#	define INIT_STUB(x_pin) { \
		gpio_init(x_pin); gpio_set_dir(x_pin, GPIO_IN); \
		gpio_init(x_pin + 1); gpio_set_dir(x_pin + 1, GPIO_IN); \
	}

	if (PH_O_IS_KBD_PS2) {
		kb_init(_KBD_DATA_PIN);
	} else {
		INIT_STUB(_KBD_DATA_PIN);
	}

	if (PH_O_IS_MOUSE_PS2) {
		ms_init(_MOUSE_DATA_PIN);
	} else {
		INIT_STUB(_MOUSE_DATA_PIN);
	}

#	undef INIT_STUB
}

void ph_ps2_task(void) {
	if (PH_O_IS_KBD_PS2) {
		ph_g_ps2_kbd_online = kb_task();
	}

	if (PH_O_IS_MOUSE_PS2) {
		ph_g_ps2_mouse_online = ms_task();
	}
}

void ph_ps2_kbd_send_key(u8 key, bool state) {
	if (PH_O_IS_KBD_PS2) {
		if (key >= 0xe0 && key <= 0xe7) {
			if (state) {
				ph_ps2_kbd_modifiers = ph_ps2_kbd_modifiers | (1 << (key - 0xe0));
			} else {
				ph_ps2_kbd_modifiers = ph_ps2_kbd_modifiers & ~(1 << (key - 0xe0));
			}
		}

		kb_send_key(key, state, ph_ps2_kbd_modifiers);
	}
}

void ph_ps2_mouse_send_button(u8 button, bool state) {
	if (PH_O_IS_MOUSE_PS2) {
		button--;

		if (state) {
			ph_ps2_mouse_buttons = ph_ps2_mouse_buttons | (1 << button);
		} else {
			ph_ps2_mouse_buttons = ph_ps2_mouse_buttons & ~(1 << button);
		}

		ms_send_packet(ph_ps2_mouse_buttons, 0, 0, 0, 0);
	}
}

void ph_ps2_mouse_send_rel(s8 x, s8 y) {
	if (PH_O_IS_MOUSE_PS2) {
		ms_send_packet(ph_ps2_mouse_buttons, x, y, 0, 0);
	}
}

void ph_ps2_mouse_send_wheel(s8 h, s8 v) {
	if (PH_O_IS_MOUSE_PS2) {
		ms_send_packet(ph_ps2_mouse_buttons, 0, 0, h, v);
	}
}

void ph_ps2_send_clear(void) {
	if (PH_O_IS_KBD_PS2) {
		for(u8 key = 0xe0; key <= 0xe7; key++) {
			kb_send_key(key, false, 0);
		}

		for(u8 key = 4; key <= 116; key++) {
			kb_send_key(key, false, 0);
		}
	}

	if (PH_O_IS_MOUSE_PS2) {
		ms_send_packet(0, 0, 0, 0, 0);
	}
}
