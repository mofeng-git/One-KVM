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


u8 ph_g_ps2_kbd_leds;
bool ph_g_ps2_kbd_online;
bool ph_g_ps2_mouse_online;


void ph_ps2_init(void) {
	// TODO: PS2: Initialize PS/2 stuff here IF you have at least one PS/2 device, check ph_usb.c for the example
	// Use macro PH_O_IS_KBD_PS2 and PH_O_IS_MOUSE_PS2
	if (PH_O_IS_KBD_PS2 || PH_O_IS_MOUSE_PS2) {
		// ...
	}
}

void ph_ps2_task(void) {
	// TODO: PS2: Perform periodic stuff here IF you have at least one PS/2 device, check ph_usb.c
	if (PH_O_IS_KBD_PS2 || PH_O_IS_MOUSE_PS2) {
		// ...
	}
	// Here you should update some values:
	//   - ph_g_ps2_kbd_leds - keyboard LEDs mask like on USB
	//   - ph_g_ps2_kbd_online - if keyboard online (by clock?)
	//   - ph_g_ps2_mouse_online if mouse online (by clock?)
	// It is important not to have ANY sleep() call inside it.
	// There should also be no freezes if the keyboard or mouse is not available.
}

void ph_ps2_kbd_send_key(u8 key, bool state) {
	// TODO: PS2: Send keyboard key
	//   @key - is a USB keycode, modifier keys has range 0xE0...0xE7, check ph_usb_kbd_send_key()
	//   @state - true if pressed, false if released
	// The function should take care not to send duplicate events (if needed for PS/2)
	// If the PS2 keyboard is not used (PH_O_IS_KBD_PS2 is false), the function should do nothing.
	(void)key; // Remove this
	(void)state; // Remove this
}

void ph_ps2_mouse_send_button(u8 button, bool state) {
	// TODO: PS2: Send mouse button
	//   @button - USB button code
	//   @state - true if pressed, false if released
	// The function should take care not to send duplicate events (if needed for PS/2)
	// If the PS2 keyboard is not used (PH_O_IS_MOUSE_PS2 is false), the function should do nothing.
	(void)button; // Remove this
	(void)state; // Remove this
}

void ph_ps2_mouse_send_rel(s8 x, s8 y) {
	// TODO: PS2: Send relative move event
	// If the PS2 keyboard is not used (PH_O_IS_MOUSE_PS2 is false), the function should do nothing.
	(void)x; // Remove this
	(void)y; // Remove this
}

void ph_ps2_mouse_send_wheel(s8 h, s8 v) {
	(void)h;
	// TODO: PS2: Send wheel. As I understand, PS/2 has no horizontal scrolling, so @h just can be ignored.
	//   @v - vertical scrolling like on USB
	// If the PS2 keyboard is not used (PH_O_IS_MOUSE_PS2 is false), the function should do nothing.
	(void)v; // Remove this
}

void ph_ps2_send_clear(void) {
	// TODO: PS2: Release all pressed buttons and keys.
	// If PH_O_IS_KBD_PS2, release all PS/2 buttons
	// also if PH_O_IS_MOUSE_PS2 is true, release all mouse buttons
}
