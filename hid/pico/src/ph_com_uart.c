/*****************************************************************************
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
*****************************************************************************/


#include "ph_com_uart.h"

#include "pico/stdlib.h"
#include "hardware/gpio.h"
#include "hardware/uart.h"

#include "ph_types.h"


#define _BUS		uart1
#define _SPEED		115200
#define _RX_PIN		21
#define _TX_PIN		20
#define _TIMEOUT_US	100000


static u8 _buf[8] = {0};
static u8 _index = 0;
static u64 _last_ts = 0;

static void (*_data_cb)(const u8 *) = NULL;
static void (*_timeout_cb)(void) = NULL;


void ph_com_uart_init(void (*data_cb)(const u8 *), void (*timeout_cb)(void)) {
	_data_cb = data_cb;
	_timeout_cb = timeout_cb;
	uart_init(_BUS, _SPEED);
	gpio_set_function(_RX_PIN, GPIO_FUNC_UART);
	gpio_set_function(_TX_PIN, GPIO_FUNC_UART);
}

void ph_com_uart_task(void) {
	if (uart_is_readable(_BUS)) {
		_buf[_index] = (u8)uart_getc(_BUS);
		if (_index == 7) {
			_data_cb(_buf);
			_index = 0;
		} else {
			_last_ts = time_us_64();
			++_index;
		}
	} else if (_index > 0) {
		if (_last_ts + _TIMEOUT_US < time_us_64()) {
			_timeout_cb();
			_index = 0;
		}
	}
}

void ph_com_uart_write(const u8 *data) {
	uart_write_blocking(_BUS, data, 8);
}
