/*****************************************************************************
#                                                                            #
#    KVMD - The main PiKVM daemon.                                           #
#                                                                            #
#    Copyright (C) 2018-2021  Maxim Devaev <mdevaev@gmail.com>               #
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

#include <HID-Project.h>


KeyboardKeycode keymapUsb(uint8_t code) {
	switch (code) {
		case 1: return KEY_A;
		case 2: return KEY_B;
		case 3: return KEY_C;
		case 4: return KEY_D;
		case 5: return KEY_E;
		case 6: return KEY_F;
		case 7: return KEY_G;
		case 8: return KEY_H;
		case 9: return KEY_I;
		case 10: return KEY_J;
		case 11: return KEY_K;
		case 12: return KEY_L;
		case 13: return KEY_M;
		case 14: return KEY_N;
		case 15: return KEY_O;
		case 16: return KEY_P;
		case 17: return KEY_Q;
		case 18: return KEY_R;
		case 19: return KEY_S;
		case 20: return KEY_T;
		case 21: return KEY_U;
		case 22: return KEY_V;
		case 23: return KEY_W;
		case 24: return KEY_X;
		case 25: return KEY_Y;
		case 26: return KEY_Z;
		case 27: return KEY_1;
		case 28: return KEY_2;
		case 29: return KEY_3;
		case 30: return KEY_4;
		case 31: return KEY_5;
		case 32: return KEY_6;
		case 33: return KEY_7;
		case 34: return KEY_8;
		case 35: return KEY_9;
		case 36: return KEY_0;
		case 37: return KEY_ENTER;
		case 38: return KEY_ESC;
		case 39: return KEY_BACKSPACE;
		case 40: return KEY_TAB;
		case 41: return KEY_SPACE;
		case 42: return KEY_MINUS;
		case 43: return KEY_EQUAL;
		case 44: return KEY_LEFT_BRACE;
		case 45: return KEY_RIGHT_BRACE;
		case 46: return KEY_BACKSLASH;
		case 47: return KEY_SEMICOLON;
		case 48: return KEY_QUOTE;
		case 49: return KEY_TILDE;
		case 50: return KEY_COMMA;
		case 51: return KEY_PERIOD;
		case 52: return KEY_SLASH;
		case 53: return KEY_CAPS_LOCK;
		case 54: return KEY_F1;
		case 55: return KEY_F2;
		case 56: return KEY_F3;
		case 57: return KEY_F4;
		case 58: return KEY_F5;
		case 59: return KEY_F6;
		case 60: return KEY_F7;
		case 61: return KEY_F8;
		case 62: return KEY_F9;
		case 63: return KEY_F10;
		case 64: return KEY_F11;
		case 65: return KEY_F12;
		case 66: return KEY_PRINT;
		case 67: return KEY_INSERT;
		case 68: return KEY_HOME;
		case 69: return KEY_PAGE_UP;
		case 70: return KEY_DELETE;
		case 71: return KEY_END;
		case 72: return KEY_PAGE_DOWN;
		case 73: return KEY_RIGHT_ARROW;
		case 74: return KEY_LEFT_ARROW;
		case 75: return KEY_DOWN_ARROW;
		case 76: return KEY_UP_ARROW;
		case 77: return KEY_LEFT_CTRL;
		case 78: return KEY_LEFT_SHIFT;
		case 79: return KEY_LEFT_ALT;
		case 80: return KEY_LEFT_GUI;
		case 81: return KEY_RIGHT_CTRL;
		case 82: return KEY_RIGHT_SHIFT;
		case 83: return KEY_RIGHT_ALT;
		case 84: return KEY_RIGHT_GUI;
		case 85: return KEY_PAUSE;
		case 86: return KEY_SCROLL_LOCK;
		case 87: return KEY_NUM_LOCK;
		case 88: return KEY_MENU;
		case 89: return KEYPAD_DIVIDE;
		case 90: return KEYPAD_MULTIPLY;
		case 91: return KEYPAD_SUBTRACT;
		case 92: return KEYPAD_ADD;
		case 93: return KEYPAD_ENTER;
		case 94: return KEYPAD_1;
		case 95: return KEYPAD_2;
		case 96: return KEYPAD_3;
		case 97: return KEYPAD_4;
		case 98: return KEYPAD_5;
		case 99: return KEYPAD_6;
		case 100: return KEYPAD_7;
		case 101: return KEYPAD_8;
		case 102: return KEYPAD_9;
		case 103: return KEYPAD_0;
		case 104: return KEYPAD_DOT;
		case 105: return KEY_POWER;
		case 106: return KEY_NON_US;
		case 107: return KEY_INTERNATIONAL3;
		default: return KEY_ERROR_UNDEFINED;
	}
}
