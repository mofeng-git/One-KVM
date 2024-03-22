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
		case 1: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 28; return; // KeyA
		case 2: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 50; return; // KeyB
		case 3: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 33; return; // KeyC
		case 4: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 35; return; // KeyD
		case 5: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 36; return; // KeyE
		case 6: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 43; return; // KeyF
		case 7: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 52; return; // KeyG
		case 8: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 51; return; // KeyH
		case 9: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 67; return; // KeyI
		case 10: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 59; return; // KeyJ
		case 11: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 66; return; // KeyK
		case 12: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 75; return; // KeyL
		case 13: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 58; return; // KeyM
		case 14: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 49; return; // KeyN
		case 15: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 68; return; // KeyO
		case 16: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 77; return; // KeyP
		case 17: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 21; return; // KeyQ
		case 18: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 45; return; // KeyR
		case 19: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 27; return; // KeyS
		case 20: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 44; return; // KeyT
		case 21: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 60; return; // KeyU
		case 22: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 42; return; // KeyV
		case 23: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 29; return; // KeyW
		case 24: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 34; return; // KeyX
		case 25: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 53; return; // KeyY
		case 26: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 26; return; // KeyZ
		case 27: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 22; return; // Digit1
		case 28: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 30; return; // Digit2
		case 29: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 38; return; // Digit3
		case 30: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 37; return; // Digit4
		case 31: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 46; return; // Digit5
		case 32: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 54; return; // Digit6
		case 33: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 61; return; // Digit7
		case 34: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 62; return; // Digit8
		case 35: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 70; return; // Digit9
		case 36: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 69; return; // Digit0
		case 37: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 90; return; // Enter
		case 38: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 118; return; // Escape
		case 39: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 102; return; // Backspace
		case 40: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 13; return; // Tab
		case 41: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 41; return; // Space
		case 42: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 78; return; // Minus
		case 43: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 85; return; // Equal
		case 44: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 84; return; // BracketLeft
		case 45: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 91; return; // BracketRight
		case 46: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 93; return; // Backslash
		case 47: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 76; return; // Semicolon
		case 48: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 82; return; // Quote
		case 49: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 14; return; // Backquote
		case 50: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 65; return; // Comma
		case 51: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 73; return; // Period
		case 52: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 74; return; // Slash
		case 53: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 88; return; // CapsLock
		case 54: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 5; return; // F1
		case 55: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 6; return; // F2
		case 56: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 4; return; // F3
		case 57: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 12; return; // F4
		case 58: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 3; return; // F5
		case 59: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 11; return; // F6
		case 60: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 131; return; // F7
		case 61: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 10; return; // F8
		case 62: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 1; return; // F9
		case 63: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 9; return; // F10
		case 64: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 120; return; // F11
		case 65: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 7; return; // F12
		case 66: *ps2_type = PS2_KEY_TYPE_PRINT; *ps2_code = 255; return; // PrintScreen
		case 67: *ps2_type = PS2_KEY_TYPE_SPEC; *ps2_code = 112; return; // Insert
		case 68: *ps2_type = PS2_KEY_TYPE_SPEC; *ps2_code = 108; return; // Home
		case 69: *ps2_type = PS2_KEY_TYPE_SPEC; *ps2_code = 125; return; // PageUp
		case 70: *ps2_type = PS2_KEY_TYPE_SPEC; *ps2_code = 113; return; // Delete
		case 71: *ps2_type = PS2_KEY_TYPE_SPEC; *ps2_code = 105; return; // End
		case 72: *ps2_type = PS2_KEY_TYPE_SPEC; *ps2_code = 122; return; // PageDown
		case 73: *ps2_type = PS2_KEY_TYPE_SPEC; *ps2_code = 116; return; // ArrowRight
		case 74: *ps2_type = PS2_KEY_TYPE_SPEC; *ps2_code = 107; return; // ArrowLeft
		case 75: *ps2_type = PS2_KEY_TYPE_SPEC; *ps2_code = 114; return; // ArrowDown
		case 76: *ps2_type = PS2_KEY_TYPE_SPEC; *ps2_code = 117; return; // ArrowUp
		case 77: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 20; return; // ControlLeft
		case 78: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 18; return; // ShiftLeft
		case 79: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 17; return; // AltLeft
		case 80: *ps2_type = PS2_KEY_TYPE_SPEC; *ps2_code = 31; return; // MetaLeft
		case 81: *ps2_type = PS2_KEY_TYPE_SPEC; *ps2_code = 20; return; // ControlRight
		case 82: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 89; return; // ShiftRight
		case 83: *ps2_type = PS2_KEY_TYPE_SPEC; *ps2_code = 17; return; // AltRight
		case 84: *ps2_type = PS2_KEY_TYPE_SPEC; *ps2_code = 39; return; // MetaRight
		case 85: *ps2_type = PS2_KEY_TYPE_PAUSE; *ps2_code = 255; return; // Pause
		case 86: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 126; return; // ScrollLock
		case 87: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 119; return; // NumLock
		case 88: *ps2_type = PS2_KEY_TYPE_SPEC; *ps2_code = 47; return; // ContextMenu
		case 89: *ps2_type = PS2_KEY_TYPE_SPEC; *ps2_code = 74; return; // NumpadDivide
		case 90: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 124; return; // NumpadMultiply
		case 91: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 123; return; // NumpadSubtract
		case 92: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 121; return; // NumpadAdd
		case 93: *ps2_type = PS2_KEY_TYPE_SPEC; *ps2_code = 90; return; // NumpadEnter
		case 94: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 105; return; // Numpad1
		case 95: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 114; return; // Numpad2
		case 96: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 122; return; // Numpad3
		case 97: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 107; return; // Numpad4
		case 98: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 115; return; // Numpad5
		case 99: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 116; return; // Numpad6
		case 100: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 108; return; // Numpad7
		case 101: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 117; return; // Numpad8
		case 102: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 125; return; // Numpad9
		case 103: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 112; return; // Numpad0
		case 104: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 113; return; // NumpadDecimal
		case 105: *ps2_type = PS2_KEY_TYPE_SPEC; *ps2_code = 94; return; // Power
		case 106: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 97; return; // IntlBackslash
		case 107: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 106; return; // IntlYen
		case 108: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 81; return; // IntlRo
		case 109: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 19; return; // KanaMode
		case 110: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 100; return; // Convert
		case 111: *ps2_type = PS2_KEY_TYPE_REG; *ps2_code = 103; return; // NonConvert
	}
}
