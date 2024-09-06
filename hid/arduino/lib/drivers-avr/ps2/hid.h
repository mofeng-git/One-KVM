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


#pragma once

#include <Arduino.h>
#include <ps2dev.h>

#include "keyboard.h"
#include "keymap.h"

// #define HID_PS2_KBD_CLOCK_PIN	7
// #define HID_PS2_KBD_DATA_PIN		5


class Ps2Keyboard : public DRIVERS::Keyboard {
	// https://wiki.osdev.org/PS/2_Keyboard

	public:
		Ps2Keyboard() : DRIVERS::Keyboard(DRIVERS::PS2_KEYBOARD), _dev(HID_PS2_KBD_CLOCK_PIN, HID_PS2_KBD_DATA_PIN) {}

		void begin() override {
			_dev.keyboard_init();
		}

		void periodic() override {
			_dev.keyboard_handle(&_leds);
		}

		void sendKey(uint8_t code, bool state) override {
			Ps2KeyType ps2_type;
			uint8_t ps2_code;

			keymapPs2(code, &ps2_type, &ps2_code);
			if (ps2_type != PS2_KEY_TYPE_UNKNOWN) {
				// Не отправлялась часть нажатий. Когда clock на нуле, комп не принимает ничего от клавы.
				// Этот костыль понижает процент пропущенных нажатий.
				while (digitalRead(HID_PS2_KBD_CLOCK_PIN) == 0) {};
				if (state) {
					switch (ps2_type) {
						case PS2_KEY_TYPE_REG: _dev.keyboard_press(ps2_code); break;
						case PS2_KEY_TYPE_SPEC: _dev.keyboard_press_special(ps2_code); break;
						case PS2_KEY_TYPE_PRINT: _dev.keyboard_press_printscreen(); break;
						case PS2_KEY_TYPE_PAUSE: _dev.keyboard_pausebreak(); break;
						case PS2_KEY_TYPE_UNKNOWN: break;
					}
				} else {
					switch (ps2_type) {
						case PS2_KEY_TYPE_REG: _dev.keyboard_release(ps2_code); break;
						case PS2_KEY_TYPE_SPEC: _dev.keyboard_release_special(ps2_code); break;
						case PS2_KEY_TYPE_PRINT: _dev.keyboard_release_printscreen(); break;
						case PS2_KEY_TYPE_PAUSE: break;
						case PS2_KEY_TYPE_UNKNOWN: break;
					}
				}
			}
		}

		bool isOffline() override {
			return false;
		}

		KeyboardLedsState getLeds() override {
			periodic();
			KeyboardLedsState result = {
				.caps = _leds & 0b00000100,
				.scroll = _leds & 0b00000001,
				.num = _leds & 0b00000010,
			};
			return result;
		}

	private:
		PS2dev _dev;
		uint8_t _leds = 0;
};
