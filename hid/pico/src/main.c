/*****************************************************************************
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
*****************************************************************************/


#include "pico/stdlib.h"
#include "hardware/gpio.h"
#include "hardware/watchdog.h"

#include "ph_types.h"
#include "ph_tools.h"
#include "ph_outputs.h"
#include "ph_usb.h"
#include "ph_spi.h"
#include "ph_uart.h"
#include "ph_proto.h"
#include "ph_cmds.h"
#include "ph_debug.h"


#define _COMM_PIN 22


static bool _comm_use_spi = true;
#define _COMM(x_func, ...) { \
		if (_comm_use_spi) { ph_spi_##x_func(__VA_ARGS__); } \
		else { ph_uart_##x_func(__VA_ARGS__); } \
	}

static bool _reset_required = false;


static u8 _handle_request(const u8 *data) { // 8 bytes
	// FIXME: See kvmd/kvmd#80
	// Should input buffer be cleared in this case?
	if (data[0] == PH_PROTO_MAGIC && ph_crc16(data, 6) == ph_merge8_u16(data[6], data[7])) {
#		define HANDLE(x_handler, x_reset) { \
				x_handler(data + 2); \
				if (x_reset) { _reset_required = true; } \
				return PH_PROTO_PONG_OK; \
			}
		switch (data[1]) {
			case PH_PROTO_CMD_PING:				return PH_PROTO_PONG_OK;
			case PH_PROTO_CMD_SET_KBD:			HANDLE(ph_cmd_set_kbd, true);
			case PH_PROTO_CMD_SET_MOUSE:		HANDLE(ph_cmd_set_mouse, true);
			case PH_PROTO_CMD_SET_CONNECTED:	return PH_PROTO_PONG_OK; // Arduino AUM
			case PH_PROTO_CMD_CLEAR_HID:		HANDLE(ph_cmd_send_clear, false);
			case PH_PROTO_CMD_KBD_KEY:			HANDLE(ph_cmd_kbd_send_key, false);
			case PH_PROTO_CMD_MOUSE_BUTTON:		HANDLE(ph_cmd_mouse_send_button, false);
			case PH_PROTO_CMD_MOUSE_ABS:		HANDLE(ph_cmd_mouse_send_abs, false);
			case PH_PROTO_CMD_MOUSE_REL:		HANDLE(ph_cmd_mouse_send_rel, false);
			case PH_PROTO_CMD_MOUSE_WHEEL:		HANDLE(ph_cmd_mouse_send_wheel, false);
			case PH_PROTO_CMD_REPEAT:			return 0;
		}
#		undef HANDLE
		return PH_PROTO_RESP_INVALID_ERROR;
	}
	return PH_PROTO_RESP_CRC_ERROR;
}

static void _send_response(u8 code) {
	static u8 prev_code = PH_PROTO_RESP_NONE;
	if (code == 0) {
		code = prev_code; // Repeat the last code
	} else {
		prev_code = code;
	}

	u8 resp[8] = {0};
	resp[0] = PH_PROTO_MAGIC_RESP;

	if (code & PH_PROTO_PONG_OK) {
		resp[1] = PH_PROTO_PONG_OK;
		if (_reset_required) {
			resp[1] |= PH_PROTO_PONG_RESET_REQUIRED;
		}
		resp[2] = PH_PROTO_OUT1_DYNAMIC;

		resp[1] |= ph_cmd_get_offlines();
		resp[1] |= ph_cmd_kbd_get_leds();
		resp[2] |= ph_g_outputs_active;
		resp[3] |= ph_g_outputs_avail;
	} else {
		resp[1] = code;
	}

	ph_split16(ph_crc16(resp, 6), &resp[6], &resp[7]);

	_COMM(write, resp);

	if (_reset_required) {
		watchdog_reboot(0, 0, 100); // Даем немного времени чтобы отправить ответ, а потом ребутимся
	}
}

static void _data_handler(const u8 *data) {
	_send_response(_handle_request(data));
}

static void _timeout_handler(void) {
	_send_response(PH_PROTO_RESP_TIMEOUT_ERROR);
}


int main(void) {
	ph_debug_init(false); // No UART
	ph_outputs_init();
	ph_usb_init();

	gpio_init(_COMM_PIN);
	gpio_set_dir(_COMM_PIN, GPIO_IN);
	gpio_pull_up(_COMM_PIN);
	_comm_use_spi = !gpio_get(_COMM_PIN);
	_COMM(init, _data_handler, _timeout_handler);

	while (true) {
		ph_usb_task();
		if (!_reset_required) {
			_COMM(task);
			ph_debug_act_pulse(100);
		}
	}
	return 0;
}
