/* ========================================================================= #
#                                                                            #
#    KVMD - The main PiKVM daemon.                                           #
#                                                                            #
#    Copyright (C) 2018-2023  Maxim Devaev <mdevaev@gmail.com>               #
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


void ph_ps2_init(void) {
	if (PH_O_HAS_PS2) {
		gpio_init(_LS_POWER_PIN);
		gpio_set_dir(_LS_POWER_PIN, GPIO_OUT);
		gpio_put(_LS_POWER_PIN, true);
	}
	
	if (PH_O_IS_KBD_PS2) {
		ph_ps2_kbd_init(_KBD_DATA_PIN);
	}
	
	if (PH_O_IS_MOUSE_PS2) {
		ph_ps2_mouse_init(_MOUSE_DATA_PIN);
	}
}

void ph_ps2_task(void) {
	if (PH_O_IS_KBD_PS2) {
		ph_ps2_kbd_task();
	}
	
	if (PH_O_IS_MOUSE_PS2) {
		ph_ps2_mouse_task();
	}
}

void ph_ps2_send_clear(void) {
	// TODO: PS2: Release all pressed buttons and keys.
	// If PH_O_IS_KBD_PS2, release all PS/2 buttons
	// also if PH_O_IS_MOUSE_PS2 is true, release all mouse buttons
}
