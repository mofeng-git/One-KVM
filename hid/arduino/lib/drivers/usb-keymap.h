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


uint8_t keymapUsb(uint8_t code) {
	switch (code) {
		case 1: return 4; // KeyA
		case 2: return 5; // KeyB
		case 3: return 6; // KeyC
		case 4: return 7; // KeyD
		case 5: return 8; // KeyE
		case 6: return 9; // KeyF
		case 7: return 10; // KeyG
		case 8: return 11; // KeyH
		case 9: return 12; // KeyI
		case 10: return 13; // KeyJ
		case 11: return 14; // KeyK
		case 12: return 15; // KeyL
		case 13: return 16; // KeyM
		case 14: return 17; // KeyN
		case 15: return 18; // KeyO
		case 16: return 19; // KeyP
		case 17: return 20; // KeyQ
		case 18: return 21; // KeyR
		case 19: return 22; // KeyS
		case 20: return 23; // KeyT
		case 21: return 24; // KeyU
		case 22: return 25; // KeyV
		case 23: return 26; // KeyW
		case 24: return 27; // KeyX
		case 25: return 28; // KeyY
		case 26: return 29; // KeyZ
		case 27: return 30; // Digit1
		case 28: return 31; // Digit2
		case 29: return 32; // Digit3
		case 30: return 33; // Digit4
		case 31: return 34; // Digit5
		case 32: return 35; // Digit6
		case 33: return 36; // Digit7
		case 34: return 37; // Digit8
		case 35: return 38; // Digit9
		case 36: return 39; // Digit0
		case 37: return 40; // Enter
		case 38: return 41; // Escape
		case 39: return 42; // Backspace
		case 40: return 43; // Tab
		case 41: return 44; // Space
		case 42: return 45; // Minus
		case 43: return 46; // Equal
		case 44: return 47; // BracketLeft
		case 45: return 48; // BracketRight
		case 46: return 49; // Backslash
		case 47: return 51; // Semicolon
		case 48: return 52; // Quote
		case 49: return 53; // Backquote
		case 50: return 54; // Comma
		case 51: return 55; // Period
		case 52: return 56; // Slash
		case 53: return 57; // CapsLock
		case 54: return 58; // F1
		case 55: return 59; // F2
		case 56: return 60; // F3
		case 57: return 61; // F4
		case 58: return 62; // F5
		case 59: return 63; // F6
		case 60: return 64; // F7
		case 61: return 65; // F8
		case 62: return 66; // F9
		case 63: return 67; // F10
		case 64: return 68; // F11
		case 65: return 69; // F12
		case 66: return 70; // PrintScreen
		case 67: return 73; // Insert
		case 68: return 74; // Home
		case 69: return 75; // PageUp
		case 70: return 76; // Delete
		case 71: return 77; // End
		case 72: return 78; // PageDown
		case 73: return 79; // ArrowRight
		case 74: return 80; // ArrowLeft
		case 75: return 81; // ArrowDown
		case 76: return 82; // ArrowUp
		case 77: return 224; // ControlLeft
		case 78: return 225; // ShiftLeft
		case 79: return 226; // AltLeft
		case 80: return 227; // MetaLeft
		case 81: return 228; // ControlRight
		case 82: return 229; // ShiftRight
		case 83: return 230; // AltRight
		case 84: return 231; // MetaRight
		case 85: return 72; // Pause
		case 86: return 71; // ScrollLock
		case 87: return 83; // NumLock
		case 88: return 101; // ContextMenu
		case 89: return 84; // NumpadDivide
		case 90: return 85; // NumpadMultiply
		case 91: return 86; // NumpadSubtract
		case 92: return 87; // NumpadAdd
		case 93: return 88; // NumpadEnter
		case 94: return 89; // Numpad1
		case 95: return 90; // Numpad2
		case 96: return 91; // Numpad3
		case 97: return 92; // Numpad4
		case 98: return 93; // Numpad5
		case 99: return 94; // Numpad6
		case 100: return 95; // Numpad7
		case 101: return 96; // Numpad8
		case 102: return 97; // Numpad9
		case 103: return 98; // Numpad0
		case 104: return 99; // NumpadDecimal
		case 105: return 102; // Power
		case 106: return 100; // IntlBackslash
		case 107: return 137; // IntlYen
		case 108: return 135; // IntlRo
		case 109: return 136; // KanaMode
		case 110: return 138; // Convert
		case 111: return 139; // NonConvert
		default: return 0;
	}
}
