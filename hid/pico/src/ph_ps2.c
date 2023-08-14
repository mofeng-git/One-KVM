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
#include "ph_ps2.pio.h"
#include "hardware/gpio.h"

u8 ph_g_ps2_kbd_leds = 0;
bool ph_g_ps2_kbd_online = 0;
bool ph_g_ps2_mouse_online = 0;

uint8_t const mod2ps2[] = { 0x14, 0x12, 0x11, 0x1f, 0x14, 0x59, 0x11, 0x27 };
uint8_t const hid2ps2[] = {
	0x00, 0x00, 0xfc, 0x00, 0x1c, 0x32, 0x21, 0x23, 0x24, 0x2b, 0x34, 0x33, 0x43, 0x3b, 0x42, 0x4b,
	0x3a, 0x31, 0x44, 0x4d, 0x15, 0x2d, 0x1b, 0x2c, 0x3c, 0x2a, 0x1d, 0x22, 0x35, 0x1a, 0x16, 0x1e,
	0x26, 0x25, 0x2e, 0x36, 0x3d, 0x3e, 0x46, 0x45, 0x5a, 0x76, 0x66, 0x0d, 0x29, 0x4e, 0x55, 0x54,
	0x5b, 0x5d, 0x5d, 0x4c, 0x52, 0x0e, 0x41, 0x49, 0x4a, 0x58, 0x05, 0x06, 0x04, 0x0c, 0x03, 0x0b,
	0x83, 0x0a, 0x01, 0x09, 0x78, 0x07, 0x7c, 0x7e, 0x7e, 0x70, 0x6c, 0x7d, 0x71, 0x69, 0x7a, 0x74,
	0x6b, 0x72, 0x75, 0x77, 0x4a, 0x7c, 0x7b, 0x79, 0x5a, 0x69, 0x72, 0x7a, 0x6b, 0x73, 0x74, 0x6c,
	0x75, 0x7d, 0x70, 0x71, 0x61, 0x2f, 0x37, 0x0f, 0x08, 0x10, 0x18, 0x20, 0x28, 0x30, 0x38, 0x40,
	0x48, 0x50, 0x57, 0x5f
};
uint8_t const maparray = sizeof(hid2ps2) / sizeof(uint8_t);

PIO pio = pio0;
uint sm;
uint offset;

uint16_t ph_ps2_frame(uint8_t data) {
	uint8_t parity = 1;
	for (uint8_t i = 0; i < 8; i++) {
		parity = parity ^ (data >> i & 1);
	}
	
	return ((1 << 10) | (parity << 9) | (data << 1)) ^ 0x7ff;
}

void ph_ps2_kbd_send(uint8_t data) {
	pio_sm_put(pio, sm, ph_ps2_frame(data));
}

void ph_ps2_kbd_maybe_send_e0(uint8_t data) {
	if (data == 0x46 ||
		 (data >= 0x49 && data <= 0x52) ||
			data == 0x54 || data == 0x58 ||
			data == 0x65 || data == 0x66 ||
			data >= 0x81) {
		ph_ps2_kbd_send(0xe0);
	}
}

void ph_ps2_init(void) {
	if (PH_O_IS_KBD_PS2 || PH_O_IS_MOUSE_PS2) {
		gpio_init(13);
		gpio_set_dir(13, GPIO_OUT);
		gpio_put(13, 1); // LV pull-up voltage
		
		sm = pio_claim_unused_sm(pio, true);
		offset = pio_add_program(pio, &ps2device_program);
		ps2device_program_init(pio, sm, offset, 14);
	}
}

void ph_ps2_task(void) {
	if (PH_O_IS_KBD_PS2 || PH_O_IS_MOUSE_PS2) {
		
		if (!pio_sm_is_rx_fifo_empty(pio, sm)) {
			uint32_t fifo = pio_sm_get(pio, sm);
			fifo = fifo >> 23;
			
			uint8_t parity = 1;
			for(uint8_t i = 0; i < 8; i++) {
				parity = parity ^ (fifo >> i & 1);
			}
			
			if(parity != fifo >> 8) {
				ph_ps2_kbd_send(0xfe);
				return;
			}
			
			uint8_t data = fifo;
			
			/*switch() {
				case 0xed: // CMD: Set LEDs
					
				break;
				
				case 0xf3: // CMD: Set typematic rate and delay
					
				break;
				
				default:*/
					switch(data) {
						case 0xff: // CMD: Reset
							pio_sm_clear_fifos(pio, sm);
							pio_sm_drain_tx_fifo(pio, sm);
							ph_ps2_kbd_send(0xfa);
							ph_ps2_kbd_send(0xaa);
						return;
						
						case 0xfe: // CMD: Resend
							
						return;
						
						case 0xee: // CMD: Echo
							ph_ps2_kbd_send(0xee);
						return;
						
						case 0xf2: // CMD: Identify keyboard
							ph_ps2_kbd_send(0xfa);
							ph_ps2_kbd_send(0xab);
							ph_ps2_kbd_send(0x83);
						return;
						
						case 0xf3: // CMD: Set typematic rate and delay
						case 0xed: // CMD: Set LEDs
							
						break;
						
						case 0xf4: // CMD: Enable scanning
							
						break;
						
						case 0xf5: // CMD: Disable scanning, restore default parameters
						case 0xf6: // CMD: Set default parameters
							
						break;
					}
			/*	break;
			}*/
			
			ph_ps2_kbd_send(0xfa);
		}
		
	}
	// Here you should update some values:
	//   - ph_g_ps2_kbd_leds - keyboard LEDs mask like on USB
	//   - ph_g_ps2_kbd_online - if keyboard online (by clock?)
	//   - ph_g_ps2_mouse_online if mouse online (by clock?)
	// It is important not to have ANY sleep() call inside it.
	// There should also be no freezes if the keyboard or mouse is not available.
}

void ph_ps2_kbd_send_key(u8 key, bool state) {
	if (PH_O_IS_KBD_PS2) {
		if (key >= 0xe0 && key <= 0xe7) {
			key -= 0xe0;
			if(key > 2 && key != 5) {
				ph_ps2_kbd_send(0xe0);
			}
			
			if(!state) {
				ph_ps2_kbd_send(0xf0);
			}
			
			ph_ps2_kbd_send(mod2ps2[key]);
			
		} else if (key < maparray) {
			ph_ps2_kbd_maybe_send_e0(key);
			
			if(!state) {
				ph_ps2_kbd_send(0xf0);
			}
			
			ph_ps2_kbd_send(hid2ps2[key]);
			
		}
	}
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
