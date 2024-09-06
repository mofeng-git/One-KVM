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


#include "ph_usb.h"

#include <stdlib.h>
#include <string.h>

#include "pico/stdlib.h"
#include "pico/unique_id.h"

#include "tusb.h"
#if TUD_OPT_HIGH_SPEED
#	error "High-Speed is not supported"
#endif

#include "ph_types.h"
#include "ph_outputs.h"
#include "ph_usb_kbd.h"
#include "ph_usb_mouse.h"


u8 ph_g_usb_kbd_leds = 0;
bool ph_g_usb_kbd_online = true;
bool ph_g_usb_mouse_online = true;

static int _kbd_iface = -1;
static int _mouse_iface = -1;

static u8 _kbd_mods = 0;
static u8 _kbd_keys[6] = {0};
#define _KBD_CLEAR { _kbd_mods = 0; memset(_kbd_keys, 0, 6); }

static u8 _mouse_buttons = 0;
static s16 _mouse_abs_x = 0;
static s16 _mouse_abs_y = 0;
#define _MOUSE_CLEAR { _mouse_buttons = 0; _mouse_abs_x = 0; _mouse_abs_y = 0; }


static void _kbd_sync_report(bool new);
static void _mouse_abs_send_report(s8 h, s8 v);
static void _mouse_rel_send_report(s8 x, s8 y, s8 h, s8 v);


void ph_usb_init(void) {
	if (ph_g_is_bridge || PH_O_IS_KBD_USB || PH_O_IS_MOUSE_USB) {
		tud_init(0);
	}
}

void ph_usb_task(void) {
	if (ph_g_is_bridge || PH_O_IS_KBD_USB || PH_O_IS_MOUSE_USB) {
		tud_task();

		static u64 next_ts = 0;
		const u64 now_ts = time_us_64();
		if (next_ts == 0 || now_ts >= next_ts) {
#			define CHECK_IFACE(x_dev) \
				static u64 offline_ts = 0; \
				static bool prev_online = true; \
				const bool online = (tud_ready() && tud_hid_n_ready(_##x_dev##_iface)); \
				bool force = false; \
				if (online) { \
					if (!ph_g_usb_##x_dev##_online) { \
						force = true; /* Если был переход из долгого оффлайна в онлайн */ \
					} \
					ph_g_usb_##x_dev##_online = true; \
					offline_ts = 0; \
				} else if (prev_online && !online) { \
					offline_ts = now_ts; /* Начинаем отсчет для долгого оффлайна */ \
				} else if (!prev_online && !online && offline_ts + 50000 < now_ts) { \
					ph_g_usb_##x_dev##_online = false; /* Долгий оффлайн найден */ \
				} \
				prev_online = online;

			if (_kbd_iface >= 0) {
				CHECK_IFACE(kbd);
				_kbd_sync_report(force);
			}

			if (_mouse_iface >= 0) {
				CHECK_IFACE(mouse);
				(void)force;
			}

#			undef CHECK_IFACE
			next_ts = time_us_64() + 1000; // Every 1 ms
		}
	}
}

void ph_usb_kbd_send_key(u8 key, bool state) {
	if (_kbd_iface < 0) {
		return; // Допускаем планирование нажатия, пока устройство не готово
	}

	if (key >= HID_KEY_CONTROL_LEFT && key <= HID_KEY_GUI_RIGHT) { // 0xE0...0xE7 - Modifiers
		key = 1 << (key & 0x07); // Номер означает сдвиг
		if (state) {
			_kbd_mods |= key;
		} else {
			_kbd_mods &= ~key;
		}

	} else { // Regular keys
		if (state) {
			s8 pos = -1;
			for (u8 i = 0; i < 6; ++i) {
				if (_kbd_keys[i] == key) {
					goto already_pressed;
				} else if (_kbd_keys[i] == 0) {
					pos = i;
				}
			}
			_kbd_keys[pos >= 0 ? pos : 0] = key;
			// already_pressed:
		} else {
			for (u8 i = 0; i < 6; ++i) {
				if (_kbd_keys[i] == key) {
					_kbd_keys[i] = 0;
					break;
				}
			}
		}
	}
	already_pressed: // Old GCC doesn't like ^ that label in the end of block

	_kbd_sync_report(true);
}

void ph_usb_mouse_send_button(u8 button, bool state) {
	if (!PH_O_IS_MOUSE_USB) {
		return;
	}
	if (state) {
		_mouse_buttons |= button;
	} else {
		_mouse_buttons &= ~button;
	}
	if (PH_O_IS_MOUSE_USB_ABS) {
		_mouse_abs_send_report(0, 0);
	} else { // PH_O_IS_MOUSE_USB_REL
		_mouse_rel_send_report(0, 0, 0, 0);
	}
}

void ph_usb_mouse_send_abs(s16 x, s16 y) {
	if (PH_O_IS_MOUSE_USB_ABS) {
		_mouse_abs_x = x;
		_mouse_abs_y = y;
		_mouse_abs_send_report(0, 0);
	}
}

void ph_usb_mouse_send_rel(s8 x, s8 y) {
	if (PH_O_IS_MOUSE_USB_REL) {
		_mouse_rel_send_report(x, y, 0, 0);
	}
}

void ph_usb_mouse_send_wheel(s8 h, s8 v) {
	if (PH_O_IS_MOUSE_USB_ABS) {
		_mouse_abs_send_report(h, v);
	} else { // PH_O_IS_MOUSE_USB_REL
		_mouse_rel_send_report(0, 0, h, v);
	}
}

void ph_usb_send_clear(void) {
	if (PH_O_IS_KBD_USB) {
		_KBD_CLEAR;
		_kbd_sync_report(true);
	}
	if (PH_O_IS_MOUSE_USB) {
		_MOUSE_CLEAR;
		if (PH_O_IS_MOUSE_USB_ABS) {
			_mouse_abs_send_report(0, 0);
		} else { // PH_O_IS_MOUSE_USB_REL
			_mouse_rel_send_report(0, 0, 0, 0);
		}
	}
}

//--------------------------------------------------------------------
// RAW report senders
//--------------------------------------------------------------------

static void _kbd_sync_report(bool new) {
	static bool sent = true;
	if (_kbd_iface < 0 || !PH_O_IS_KBD_USB) {
		_KBD_CLEAR;
		sent = true;
		return;
	}
	if (new) {
		sent = false;
	}
	if (!sent) {
		if (tud_suspended()) {
			tud_remote_wakeup();
			//_KBD_CLEAR;
			//sent = true;
		} else {
			sent = tud_hid_n_keyboard_report(_kbd_iface, 0, _kbd_mods, _kbd_keys);
		}
	}
}

#define _CHECK_MOUSE(x_mode) { \
		if (_mouse_iface < 0 || !PH_O_IS_MOUSE_USB_##x_mode) { _MOUSE_CLEAR; return; } \
		if (tud_suspended()) { tud_remote_wakeup(); _MOUSE_CLEAR; return; } \
	}


static void _mouse_abs_send_report(s8 h, s8 v) {
	(void)h; // Horizontal scrolling is not supported due BIOS/UEFI compatibility reasons
	_CHECK_MOUSE(ABS);
	u16 x = ((s32)_mouse_abs_x + 32768) / 2;
	u16 y = ((s32)_mouse_abs_y + 32768) / 2;
	if (PH_O_MOUSE(USB_W98)) {
		x <<= 1;
		y <<= 1;
	}
	struct TU_ATTR_PACKED {
		u8 buttons;
		u16 x;
		u16 y;
		s8 v;
	} report = {_mouse_buttons, x, y, v};
	tud_hid_n_report(_mouse_iface, 0, &report, sizeof(report));
}

static void _mouse_rel_send_report(s8 x, s8 y, s8 h, s8 v) {
	(void)h; // Horizontal scrolling is not supported due BIOS/UEFI compatibility reasons
	_CHECK_MOUSE(REL);
	struct TU_ATTR_PACKED {
		u8 buttons;
		s8 x;
		s8 y;
		s8 v;
	} report = {_mouse_buttons, x, y, v};
	tud_hid_n_report(_mouse_iface, 0, &report, sizeof(report));
}

#undef _CHECK_MOUSE


//--------------------------------------------------------------------
// Device callbacks
//--------------------------------------------------------------------

u16 tud_hid_get_report_cb(u8 iface, u8 report_id, hid_report_type_t report_type, u8 *buf, u16 len) {
	// Invoked when received GET_REPORT control request, return 0 == STALL
	(void)iface;
	(void)report_id;
	(void)report_type;
	(void)buf;
	(void)len;
	return 0;
}

void tud_hid_set_report_cb(u8 iface, u8 report_id, hid_report_type_t report_type, const u8 *buf, u16 len) {
	// Invoked when received SET_REPORT control request
	// or received data on OUT endpoint (ReportID=0, Type=0)
	(void)report_id;
	if (iface == _kbd_iface && report_type == HID_REPORT_TYPE_OUTPUT && len >= 1) {
		ph_g_usb_kbd_leds = buf[0];
	}
}

const u8 *tud_hid_descriptor_report_cb(u8 iface) {
	if ((int)iface == _mouse_iface) {
		if (PH_O_IS_MOUSE_USB_ABS) {
			return PH_USB_MOUSE_ABS_DESC;
		} else { // PH_O_IS_MOUSE_USB_REL
			return PH_USB_MOUSE_REL_DESC;
		}
	}
	return PH_USB_KBD_DESC; // _kbd_iface, PH_O_IS_KBD_USB
}

const u8 *_bridge_tud_descriptor_configuration_cb(void) {
	enum {num_cdc = 0, num_cdc_data, num_total};
	static const u8 desc[] = {
		TUD_CONFIG_DESCRIPTOR(
			1,      // Config number
			num_total,// Interface count
			0,      // String index
			(TUD_CONFIG_DESC_LEN + TUD_CDC_DESC_LEN), // Total length
			0,      // Attribute
			100     // Power in mA
		),
		TUD_CDC_DESCRIPTOR(
			num_cdc,// Interface number
			4,      // String index
			0x81,   // EPNUM_CDC_NOTIF - EP notification address
			8,      // EP notification size
			0x02,   // EPNUM_CDC_OUT - EP OUT data address
			0x82,   // EPNUM_CDC_IN - EP IN data address
			64      // EP size
		),
	};
	return desc;
}

const u8 *_hid_tud_descriptor_configuration_cb(void) {
	static u8 desc[TUD_CONFIG_DESC_LEN + TUD_HID_DESC_LEN * 2] = {0};
	static bool filled = false;

	if (!filled) {
		uz offset = TUD_CONFIG_DESC_LEN;
		u8 iface = 0;
		u8 ep = 0x81;

#		define APPEND_DESC(x_proto, x_desc, x_iface_to) { \
				const u8 part[] = {TUD_HID_DESCRIPTOR( \
					(x_iface_to = iface), /* Interface number */ \
					0, x_proto, x_desc##_LEN, /* String index, protocol, report descriptor len */ \
					ep, CFG_TUD_HID_EP_BUFSIZE, 1)}; /* EP In address, size, polling interval */ \
				memcpy(desc + offset, part, TUD_HID_DESC_LEN); \
				offset += TUD_HID_DESC_LEN; ++iface; ++ep; \
			}

		if (PH_O_IS_KBD_USB) {
			APPEND_DESC(HID_ITF_PROTOCOL_KEYBOARD, PH_USB_KBD_DESC, _kbd_iface);
		}
		if (PH_O_IS_MOUSE_USB_ABS) {
			APPEND_DESC(HID_ITF_PROTOCOL_NONE, PH_USB_MOUSE_ABS_DESC, _mouse_iface);
		} else if (PH_O_IS_MOUSE_USB_REL) {
			APPEND_DESC(HID_ITF_PROTOCOL_MOUSE, PH_USB_MOUSE_REL_DESC, _mouse_iface);
		}

#		undef APPEND_DESC

  		// Config number, interface count, string index, total length, attribute, power in mA
		const u8 part[] = {TUD_CONFIG_DESCRIPTOR(1, iface, 0, offset, TUSB_DESC_CONFIG_ATT_REMOTE_WAKEUP, 100)};
		memcpy(desc, part, TUD_CONFIG_DESC_LEN);
		filled = true;
	}
	return desc;
}

const u8 *tud_descriptor_configuration_cb(u8 index) {
	// Invoked when received GET CONFIGURATION DESCRIPTOR
	(void)index;
	if (ph_g_is_bridge) {
		return _bridge_tud_descriptor_configuration_cb();
	}
	return _hid_tud_descriptor_configuration_cb();
}

const u8 *tud_descriptor_device_cb(void) {
	// Invoked when received GET DEVICE DESCRIPTOR
	static tusb_desc_device_t desc = {
		.bLength			= sizeof(tusb_desc_device_t),
		.bDescriptorType	= TUSB_DESC_DEVICE,
		.bcdUSB				= 0x0200,

		.bDeviceClass		= 0,
		.bDeviceSubClass	= 0,
		.bDeviceProtocol	= 0,

		.bMaxPacketSize0	= CFG_TUD_ENDPOINT0_SIZE,

		.idVendor			= 0x1209, // https://pid.codes/org/Pi-KVM
		.idProduct			= 0xEDA2,
		.bcdDevice			= 0x0100,

		.iManufacturer		= 1,
		.iProduct			= 2,
		.iSerialNumber		= 3,

		.bNumConfigurations	= 1,
	};
	if (ph_g_is_bridge) {
		desc.bDeviceClass = TUSB_CLASS_MISC;
		desc.bDeviceSubClass = MISC_SUBCLASS_COMMON;
		desc.bDeviceProtocol = MISC_PROTOCOL_IAD;
		desc.idProduct = 0xEDA3;
	}
	return (const u8 *)&desc;
}

const u16 *tud_descriptor_string_cb(u8 index, u16 lang_id) {
	// Invoked when received GET STRING DESCRIPTOR request.
	(void)lang_id;

	static u16 desc_str[32];
	uz desc_str_len;

	if (index == 0) {
		desc_str[1] = 0x0409; // Supported language is English (0x0409)
		desc_str_len = 1;
	} else {
		char str[32];
		switch (index) {
			case 1: strcpy(str, "PiKVM"); break; // Manufacturer
			case 2: strcpy(str, (ph_g_is_bridge ? "PiKVM HID Bridge" : "PiKVM HID")); break; // Product
			case 3: pico_get_unique_board_id_string(str, 32); break; // Serial
			case 4: {
					if (ph_g_is_bridge) {
						strcpy(str, "PiKVM HID Bridge CDC");
					} else {
						return NULL;
					}
				}; break;
			default: return NULL;
		}
		desc_str_len = strlen(str);
		for (uz i = 0; i < desc_str_len; ++i) {
			desc_str[i + 1] = str[i]; // Convert ASCII string into UTF-16
		}
	}

	// First byte is length (including header), second byte is string type
	desc_str[0] = (TUSB_DESC_STRING << 8) | (2 * desc_str_len + 2);
	return desc_str;
}
