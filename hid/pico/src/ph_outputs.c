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


#include "ph_outputs.h"

#include "pico/stdlib.h"
#include "hardware/gpio.h"
#include "hardware/structs/watchdog.h"

#include "ph_types.h"
#include "ph_tools.h"
#include "ph_proto.h"


#define _PS2_ENABLED_PIN		2
#define	_PS2_SET_KBD_PIN		3
#define _PS2_SET_MOUSE_PIN		4

#define _BRIDGE_MODE_PIN		5

#define _USB_DISABLED_PIN		6
#define _USB_ENABLE_W98_PIN		7
#define _USB_SET_MOUSE_REL_PIN	8
#define _USB_SET_MOUSE_W98_PIN	9


u8 ph_g_outputs_active = 0;
u8 ph_g_outputs_avail = 0;
bool ph_g_is_bridge = false;


static int _read_outputs(void);


void ph_outputs_init(void) {
#	define INIT_SWITCH(x_pin) { gpio_init(x_pin); gpio_set_dir(x_pin, GPIO_IN); gpio_pull_up(x_pin); }
	INIT_SWITCH(_PS2_ENABLED_PIN);
	INIT_SWITCH(_PS2_SET_KBD_PIN);
	INIT_SWITCH(_PS2_SET_MOUSE_PIN);

	INIT_SWITCH(_BRIDGE_MODE_PIN);

	INIT_SWITCH(_USB_DISABLED_PIN);
	INIT_SWITCH(_USB_ENABLE_W98_PIN);
	INIT_SWITCH(_USB_SET_MOUSE_REL_PIN);
	INIT_SWITCH(_USB_SET_MOUSE_W98_PIN);
#	undef INIT_SWITCH
	sleep_ms(10); // Нужен небольшой слип для активации pull-up

	const bool o_ps2_enabled = !gpio_get(_PS2_ENABLED_PIN); // Note: all pins are pulled up!
	const bool o_ps2_kbd = !gpio_get(_PS2_SET_KBD_PIN);
	const bool o_ps2_mouse = !gpio_get(_PS2_SET_MOUSE_PIN);

	ph_g_is_bridge = !gpio_get(_BRIDGE_MODE_PIN);

	const bool o_usb_disabled = (ph_g_is_bridge || !gpio_get(_USB_DISABLED_PIN));
	const bool o_usb_enabled_w98 = !gpio_get(_USB_ENABLE_W98_PIN);
	const bool o_usb_mouse_rel = !gpio_get(_USB_SET_MOUSE_REL_PIN);
	const bool o_usb_mouse_w98 = !gpio_get(_USB_SET_MOUSE_W98_PIN);

	int outputs = _read_outputs();
	if (outputs < 0) {
		outputs = 0;

		if (o_ps2_enabled && (o_ps2_kbd || o_usb_disabled)) {
			outputs |= PH_PROTO_OUT1_KBD_PS2;
		} else if (!o_usb_disabled) {
			outputs |= PH_PROTO_OUT1_KBD_USB;
		}

		if (o_ps2_enabled && (o_ps2_mouse || o_usb_disabled)) {
			outputs |= PH_PROTO_OUT1_MOUSE_PS2;
		} else if (!o_usb_disabled) {
			if (o_usb_enabled_w98 && o_usb_mouse_w98) {
				outputs |= PH_PROTO_OUT1_MOUSE_USB_W98;
			} else if (o_usb_mouse_rel) {
				outputs |= PH_PROTO_OUT1_MOUSE_USB_REL;
			} else {
				outputs |= PH_PROTO_OUT1_MOUSE_USB_ABS;
			}
		}

		ph_outputs_write(0xFF, outputs, true);
	}

	if (!o_usb_disabled) {
		ph_g_outputs_avail |= PH_PROTO_OUT2_HAS_USB;
		if (o_usb_enabled_w98) {
			ph_g_outputs_avail |= PH_PROTO_OUT2_HAS_USB_W98;
		}
	}
	if (o_ps2_enabled) {
		ph_g_outputs_avail |= PH_PROTO_OUT2_HAS_PS2;
	}

	ph_g_outputs_active = outputs & 0xFF;
}

void ph_outputs_write(u8 mask, u8 outputs, bool force) {
	int old = 0;
	if (!force) {
		old = _read_outputs();
		if (old < 0) {
			old = 0;
		}
	}
	u8 data[4] = {0};
	data[0] = PH_PROTO_MAGIC;
	data[1] = (old & ~mask) | outputs;
	ph_split16(ph_crc16(data, 2), &data[2], &data[3]);
	const u32 s0 = ((u32)data[0] << 24) | ((u32)data[1] << 16) | ((u32)data[2] << 8) | (u32)data[3];
	watchdog_hw->scratch[0] = s0;
}

static int _read_outputs(void) {
	const u32 s0 = watchdog_hw->scratch[0];
	const u8 data[4] = {
		(s0 >> 24) & 0xFF,
		(s0 >> 16) & 0xFF,
		(s0 >> 8) & 0xFF,
		s0 & 0xFF,
	};
	if (data[0] != PH_PROTO_MAGIC || ph_crc16(data, 2) != ph_merge8_u16(data[2], data[3])) {
		return -1;
	}
	return data[1];
}
