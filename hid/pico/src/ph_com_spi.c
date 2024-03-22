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


#include "ph_com_spi.h"

#include "hardware/gpio.h"
#include "hardware/irq.h"
#include "hardware/spi.h"
#include "hardware/regs/spi.h"

#include "ph_types.h"


#define _BUS		spi0
#define _IRQ		SPI0_IRQ
#define _FREQ		(2 * 1000 * 1000)
#define _CS_PIN		21
#define _RX_PIN		20
#define _TX_PIN		19
#define _CLK_PIN	18


static volatile u8 _in_buf[8] = {0};
static volatile u8 _in_index = 0;

static volatile u8 _out_buf[8] = {0};
static volatile u8 _out_index = 0;

static void (*_data_cb)(const u8 *) = NULL;


static void _xfer_isr(void);


void ph_com_spi_init(void (*data_cb)(const u8 *), void (*timeout_cb)(void)) {
	_data_cb = data_cb;
	(void)timeout_cb;

	spi_init(_BUS, _FREQ);
	spi_set_slave(_BUS, true);
	spi_set_format(_BUS, 8, SPI_CPOL_0, SPI_CPHA_0, SPI_MSB_FIRST);

	gpio_set_function(_CS_PIN, GPIO_FUNC_SPI);
	gpio_set_function(_RX_PIN, GPIO_FUNC_SPI);
	gpio_set_function(_TX_PIN, GPIO_FUNC_SPI);
	gpio_set_function(_CLK_PIN, GPIO_FUNC_SPI);

	// https://github.com/raspberrypi/pico-sdk/blob/master/src/rp2040/hardware_regs/include/hardware/regs/spi.h
	irq_set_exclusive_handler(_IRQ, _xfer_isr);
	spi_get_hw(_BUS)->imsc = SPI_SSPIMSC_RXIM_BITS | SPI_SSPIMSC_TXIM_BITS;
	irq_set_enabled(_IRQ, true);
}

void ph_com_spi_task(void) {
	if (!_out_buf[0] && _in_index == 8) {
		_data_cb((const u8 *)_in_buf);
	}
}

void ph_com_spi_write(const u8 *data) {
	// Меджик в нулевом байте разрешает начать ответ
	for (s8 i = 7; i >= 0; --i) {
		_out_buf[i] = data[i];
	}
}

void __isr __not_in_flash_func(_xfer_isr)(void) {
#	define SR (spi_get_hw(_BUS)->sr)
#	define DR (spi_get_hw(_BUS)->dr)

	while (SR & SPI_SSPSR_TNF_BITS) {
		if (_out_buf[0] && _out_index < 8) {
			DR = (u32)_out_buf[_out_index];
			++_out_index;
			if (_out_index == 8) {
				_out_index = 0;
				_in_index = 0;
				_out_buf[0] = 0;
			}
		} else {
			DR = (u32)0;
		}
	}

	while (SR & SPI_SSPSR_RNE_BITS) {
		static bool receiving = false;
		const u8 in = DR;
		if (!receiving && in != 0) {
			receiving = true;
		}
		if (receiving && _in_index < 8) {
			_in_buf[_in_index] = in;
			++_in_index;
		}
		if (_in_index == 8) {
			receiving = false;
		}
	}

#	undef DR
#	undef SR
}
