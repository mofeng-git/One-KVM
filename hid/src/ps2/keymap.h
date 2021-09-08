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


enum Ps2KeyType : uint8_t {
	PS2_KEY_TYPE_UNKNOWN = 0,
	PS2_KEY_TYPE_REG = 1,
	PS2_KEY_TYPE_SPEC = 2,
	PS2_KEY_TYPE_PRINT = 3,
	PS2_KEY_TYPE_PAUSE = 4,
};


void keymapPs2(uint8_t code, Ps2KeyType *ps2_type, uint8_t *ps2_code) {
	*ps2_type = PS2_KEY_TYPE_UNKNOWN;
	*ps2_code = 0;

	switch (code) {
		case 1: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 28; return; // KEY_A
		case 2: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 50; return; // KEY_B
		case 3: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 33; return; // KEY_C
		case 4: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 35; return; // KEY_D
		case 5: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 36; return; // KEY_E
		case 6: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 43; return; // KEY_F
		case 7: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 52; return; // KEY_G
		case 8: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 51; return; // KEY_H
		case 9: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 67; return; // KEY_I
		case 10: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 59; return; // KEY_J
		case 11: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 66; return; // KEY_K
		case 12: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 75; return; // KEY_L
		case 13: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 58; return; // KEY_M
		case 14: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 49; return; // KEY_N
		case 15: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 68; return; // KEY_O
		case 16: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 77; return; // KEY_P
		case 17: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 21; return; // KEY_Q
		case 18: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 45; return; // KEY_R
		case 19: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 27; return; // KEY_S
		case 20: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 44; return; // KEY_T
		case 21: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 60; return; // KEY_U
		case 22: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 42; return; // KEY_V
		case 23: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 29; return; // KEY_W
		case 24: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 34; return; // KEY_X
		case 25: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 53; return; // KEY_Y
		case 26: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 26; return; // KEY_Z
		case 27: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 22; return; // KEY_1
		case 28: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 30; return; // KEY_2
		case 29: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 38; return; // KEY_3
		case 30: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 37; return; // KEY_4
		case 31: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 46; return; // KEY_5
		case 32: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 54; return; // KEY_6
		case 33: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 61; return; // KEY_7
		case 34: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 62; return; // KEY_8
		case 35: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 70; return; // KEY_9
		case 36: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 69; return; // KEY_0
		case 37: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 90; return; // KEY_ENTER
		case 38: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 118; return; // KEY_ESC
		case 39: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 102; return; // KEY_BACKSPACE
		case 40: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 13; return; // KEY_TAB
		case 41: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 41; return; // KEY_SPACE
		case 42: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 78; return; // KEY_MINUS
		case 43: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 85; return; // KEY_EQUAL
		case 44: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 84; return; // KEY_LEFT_BRACE
		case 45: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 91; return; // KEY_RIGHT_BRACE
		case 46: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 93; return; // KEY_BACKSLASH
		case 47: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 76; return; // KEY_SEMICOLON
		case 48: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 82; return; // KEY_QUOTE
		case 49: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 14; return; // KEY_TILDE
		case 50: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 65; return; // KEY_COMMA
		case 51: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 73; return; // KEY_PERIOD
		case 52: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 74; return; // KEY_SLASH
		case 53: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 88; return; // KEY_CAPS_LOCK
		case 54: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 5; return; // KEY_F1
		case 55: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 6; return; // KEY_F2
		case 56: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 4; return; // KEY_F3
		case 57: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 12; return; // KEY_F4
		case 58: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 3; return; // KEY_F5
		case 59: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 11; return; // KEY_F6
		case 60: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 131; return; // KEY_F7
		case 61: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 10; return; // KEY_F8
		case 62: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 1; return; // KEY_F9
		case 63: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 9; return; // KEY_F10
		case 64: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 120; return; // KEY_F11
		case 65: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 7; return; // KEY_F12
		case 66: *ps2_type = PS2_KEY_TYPE_PRINT; *ps2_code = 255; return; // KEY_PRINT
		case 67: *ps2_type = PS2_KEY_TYPE_SPEC; *ps2_code = 112; return; // KEY_INSERT
		case 68: *ps2_type = PS2_KEY_TYPE_SPEC; *ps2_code = 108; return; // KEY_HOME
		case 69: *ps2_type = PS2_KEY_TYPE_SPEC; *ps2_code = 125; return; // KEY_PAGE_UP
		case 70: *ps2_type = PS2_KEY_TYPE_SPEC; *ps2_code = 113; return; // KEY_DELETE
		case 71: *ps2_type = PS2_KEY_TYPE_SPEC; *ps2_code = 105; return; // KEY_END
		case 72: *ps2_type = PS2_KEY_TYPE_SPEC; *ps2_code = 122; return; // KEY_PAGE_DOWN
		case 73: *ps2_type = PS2_KEY_TYPE_SPEC; *ps2_code = 116; return; // KEY_RIGHT_ARROW
		case 74: *ps2_type = PS2_KEY_TYPE_SPEC; *ps2_code = 107; return; // KEY_LEFT_ARROW
		case 75: *ps2_type = PS2_KEY_TYPE_SPEC; *ps2_code = 114; return; // KEY_DOWN_ARROW
		case 76: *ps2_type = PS2_KEY_TYPE_SPEC; *ps2_code = 117; return; // KEY_UP_ARROW
		case 77: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 20; return; // KEY_LEFT_CTRL
		case 78: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 18; return; // KEY_LEFT_SHIFT
		case 79: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 17; return; // KEY_LEFT_ALT
		case 80: *ps2_type = PS2_KEY_TYPE_SPEC; *ps2_code = 31; return; // KEY_LEFT_GUI
		case 81: *ps2_type = PS2_KEY_TYPE_SPEC; *ps2_code = 20; return; // KEY_RIGHT_CTRL
		case 82: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 89; return; // KEY_RIGHT_SHIFT
		case 83: *ps2_type = PS2_KEY_TYPE_SPEC; *ps2_code = 17; return; // KEY_RIGHT_ALT
		case 84: *ps2_type = PS2_KEY_TYPE_SPEC; *ps2_code = 39; return; // KEY_RIGHT_GUI
		case 85: *ps2_type = PS2_KEY_TYPE_PAUSE; *ps2_code = 255; return; // KEY_PAUSE
		case 86: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 126; return; // KEY_SCROLL_LOCK
		case 87: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 119; return; // KEY_NUM_LOCK
		case 88: *ps2_type = PS2_KEY_TYPE_SPEC; *ps2_code = 47; return; // KEY_MENU
		case 89: *ps2_type = PS2_KEY_TYPE_SPEC; *ps2_code = 74; return; // KEYPAD_DIVIDE
		case 90: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 124; return; // KEYPAD_MULTIPLY
		case 91: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 123; return; // KEYPAD_SUBTRACT
		case 92: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 121; return; // KEYPAD_ADD
		case 93: *ps2_type = PS2_KEY_TYPE_SPEC; *ps2_code = 90; return; // KEYPAD_ENTER
		case 94: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 105; return; // KEYPAD_1
		case 95: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 114; return; // KEYPAD_2
		case 96: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 122; return; // KEYPAD_3
		case 97: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 107; return; // KEYPAD_4
		case 98: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 115; return; // KEYPAD_5
		case 99: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 116; return; // KEYPAD_6
		case 100: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 108; return; // KEYPAD_7
		case 101: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 117; return; // KEYPAD_8
		case 102: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 125; return; // KEYPAD_9
		case 103: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 112; return; // KEYPAD_0
		case 104: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 113; return; // KEYPAD_DOT
		case 105: *ps2_type = PS2_KEY_TYPE_SPEC; *ps2_code = 94; return; // KEY_POWER
		case 106: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 97; return; // KEY_NON_US
		case 107: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 106; return; // KEY_INTERNATIONAL3
	}
}
