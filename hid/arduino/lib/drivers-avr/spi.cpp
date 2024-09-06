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


#include "spi.h"

#ifdef CMD_SPI


static volatile uint8_t _spi_in[8] = {0};
static volatile uint8_t _spi_in_index = 0;

static volatile uint8_t _spi_out[8] = {0};
static volatile uint8_t _spi_out_index = 0;


namespace DRIVERS {
	void Spi::begin() {
		pinMode(MISO, OUTPUT);
		SPCR = (1 << SPE) | (1 << SPIE); // Slave, SPI En, IRQ En
	}

	void Spi::periodic() {
		if (!_spi_out[0] && _spi_in_index == 8) {
			_data_cb((const uint8_t *)_spi_in, 8);
		}
	}

	void Spi::write(const uint8_t *data, size_t size) {
		// Меджик в нулевом байте разрешает начать ответ
		for (int index = 7; index >= 0; --index) {
			_spi_out[index] = data[index];
		}
	}
}

ISR(SPI_STC_vect) {
	uint8_t in = SPDR;
	if (_spi_out[0] && _spi_out_index < 8) {
		SPDR = _spi_out[_spi_out_index];
		if (!(SPSR & (1 << WCOL))) {
			++_spi_out_index;
			if (_spi_out_index == 8) {
				_spi_out_index = 0;
				_spi_in_index = 0;
				_spi_out[0] = 0;
			}
		}
	} else {
		static bool receiving = false;
		if (!receiving && in != 0) {
			receiving = true;
		}
		if (receiving && _spi_in_index < 8) {
			_spi_in[_spi_in_index] = in;
			++_spi_in_index;
		}
		if (_spi_in_index == 8) {
			receiving = false;
		}
		SPDR = 0;
	}
}

#endif
