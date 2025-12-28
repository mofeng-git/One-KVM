//! USB HID keyboard key codes mapping
//!
//! This module provides mapping between JavaScript key codes and USB HID usage codes.
//! Reference: USB HID Usage Tables 1.12, Section 10 (Keyboard/Keypad Page)

/// USB HID key codes (Usage Page 0x07)
#[allow(dead_code)]
pub mod usb {
    // Letters A-Z (0x04 - 0x1D)
    pub const KEY_A: u8 = 0x04;
    pub const KEY_B: u8 = 0x05;
    pub const KEY_C: u8 = 0x06;
    pub const KEY_D: u8 = 0x07;
    pub const KEY_E: u8 = 0x08;
    pub const KEY_F: u8 = 0x09;
    pub const KEY_G: u8 = 0x0A;
    pub const KEY_H: u8 = 0x0B;
    pub const KEY_I: u8 = 0x0C;
    pub const KEY_J: u8 = 0x0D;
    pub const KEY_K: u8 = 0x0E;
    pub const KEY_L: u8 = 0x0F;
    pub const KEY_M: u8 = 0x10;
    pub const KEY_N: u8 = 0x11;
    pub const KEY_O: u8 = 0x12;
    pub const KEY_P: u8 = 0x13;
    pub const KEY_Q: u8 = 0x14;
    pub const KEY_R: u8 = 0x15;
    pub const KEY_S: u8 = 0x16;
    pub const KEY_T: u8 = 0x17;
    pub const KEY_U: u8 = 0x18;
    pub const KEY_V: u8 = 0x19;
    pub const KEY_W: u8 = 0x1A;
    pub const KEY_X: u8 = 0x1B;
    pub const KEY_Y: u8 = 0x1C;
    pub const KEY_Z: u8 = 0x1D;

    // Numbers 1-9, 0 (0x1E - 0x27)
    pub const KEY_1: u8 = 0x1E;
    pub const KEY_2: u8 = 0x1F;
    pub const KEY_3: u8 = 0x20;
    pub const KEY_4: u8 = 0x21;
    pub const KEY_5: u8 = 0x22;
    pub const KEY_6: u8 = 0x23;
    pub const KEY_7: u8 = 0x24;
    pub const KEY_8: u8 = 0x25;
    pub const KEY_9: u8 = 0x26;
    pub const KEY_0: u8 = 0x27;

    // Control keys
    pub const KEY_ENTER: u8 = 0x28;
    pub const KEY_ESCAPE: u8 = 0x29;
    pub const KEY_BACKSPACE: u8 = 0x2A;
    pub const KEY_TAB: u8 = 0x2B;
    pub const KEY_SPACE: u8 = 0x2C;
    pub const KEY_MINUS: u8 = 0x2D;
    pub const KEY_EQUAL: u8 = 0x2E;
    pub const KEY_LEFT_BRACKET: u8 = 0x2F;
    pub const KEY_RIGHT_BRACKET: u8 = 0x30;
    pub const KEY_BACKSLASH: u8 = 0x31;
    pub const KEY_HASH: u8 = 0x32; // Non-US # and ~
    pub const KEY_SEMICOLON: u8 = 0x33;
    pub const KEY_APOSTROPHE: u8 = 0x34;
    pub const KEY_GRAVE: u8 = 0x35;
    pub const KEY_COMMA: u8 = 0x36;
    pub const KEY_PERIOD: u8 = 0x37;
    pub const KEY_SLASH: u8 = 0x38;
    pub const KEY_CAPS_LOCK: u8 = 0x39;

    // Function keys F1-F12
    pub const KEY_F1: u8 = 0x3A;
    pub const KEY_F2: u8 = 0x3B;
    pub const KEY_F3: u8 = 0x3C;
    pub const KEY_F4: u8 = 0x3D;
    pub const KEY_F5: u8 = 0x3E;
    pub const KEY_F6: u8 = 0x3F;
    pub const KEY_F7: u8 = 0x40;
    pub const KEY_F8: u8 = 0x41;
    pub const KEY_F9: u8 = 0x42;
    pub const KEY_F10: u8 = 0x43;
    pub const KEY_F11: u8 = 0x44;
    pub const KEY_F12: u8 = 0x45;

    // Special keys
    pub const KEY_PRINT_SCREEN: u8 = 0x46;
    pub const KEY_SCROLL_LOCK: u8 = 0x47;
    pub const KEY_PAUSE: u8 = 0x48;
    pub const KEY_INSERT: u8 = 0x49;
    pub const KEY_HOME: u8 = 0x4A;
    pub const KEY_PAGE_UP: u8 = 0x4B;
    pub const KEY_DELETE: u8 = 0x4C;
    pub const KEY_END: u8 = 0x4D;
    pub const KEY_PAGE_DOWN: u8 = 0x4E;
    pub const KEY_RIGHT_ARROW: u8 = 0x4F;
    pub const KEY_LEFT_ARROW: u8 = 0x50;
    pub const KEY_DOWN_ARROW: u8 = 0x51;
    pub const KEY_UP_ARROW: u8 = 0x52;

    // Numpad
    pub const KEY_NUM_LOCK: u8 = 0x53;
    pub const KEY_NUMPAD_DIVIDE: u8 = 0x54;
    pub const KEY_NUMPAD_MULTIPLY: u8 = 0x55;
    pub const KEY_NUMPAD_MINUS: u8 = 0x56;
    pub const KEY_NUMPAD_PLUS: u8 = 0x57;
    pub const KEY_NUMPAD_ENTER: u8 = 0x58;
    pub const KEY_NUMPAD_1: u8 = 0x59;
    pub const KEY_NUMPAD_2: u8 = 0x5A;
    pub const KEY_NUMPAD_3: u8 = 0x5B;
    pub const KEY_NUMPAD_4: u8 = 0x5C;
    pub const KEY_NUMPAD_5: u8 = 0x5D;
    pub const KEY_NUMPAD_6: u8 = 0x5E;
    pub const KEY_NUMPAD_7: u8 = 0x5F;
    pub const KEY_NUMPAD_8: u8 = 0x60;
    pub const KEY_NUMPAD_9: u8 = 0x61;
    pub const KEY_NUMPAD_0: u8 = 0x62;
    pub const KEY_NUMPAD_DECIMAL: u8 = 0x63;

    // Additional keys
    pub const KEY_NON_US_BACKSLASH: u8 = 0x64;
    pub const KEY_APPLICATION: u8 = 0x65; // Context menu
    pub const KEY_POWER: u8 = 0x66;
    pub const KEY_NUMPAD_EQUAL: u8 = 0x67;

    // F13-F24
    pub const KEY_F13: u8 = 0x68;
    pub const KEY_F14: u8 = 0x69;
    pub const KEY_F15: u8 = 0x6A;
    pub const KEY_F16: u8 = 0x6B;
    pub const KEY_F17: u8 = 0x6C;
    pub const KEY_F18: u8 = 0x6D;
    pub const KEY_F19: u8 = 0x6E;
    pub const KEY_F20: u8 = 0x6F;
    pub const KEY_F21: u8 = 0x70;
    pub const KEY_F22: u8 = 0x71;
    pub const KEY_F23: u8 = 0x72;
    pub const KEY_F24: u8 = 0x73;

    // Modifier keys (these are handled separately in the modifier byte)
    pub const KEY_LEFT_CTRL: u8 = 0xE0;
    pub const KEY_LEFT_SHIFT: u8 = 0xE1;
    pub const KEY_LEFT_ALT: u8 = 0xE2;
    pub const KEY_LEFT_META: u8 = 0xE3;
    pub const KEY_RIGHT_CTRL: u8 = 0xE4;
    pub const KEY_RIGHT_SHIFT: u8 = 0xE5;
    pub const KEY_RIGHT_ALT: u8 = 0xE6;
    pub const KEY_RIGHT_META: u8 = 0xE7;
}

/// JavaScript key codes (event.keyCode / event.code)
#[allow(dead_code)]
pub mod js {
    // Letters
    pub const KEY_A: u8 = 65;
    pub const KEY_B: u8 = 66;
    pub const KEY_C: u8 = 67;
    pub const KEY_D: u8 = 68;
    pub const KEY_E: u8 = 69;
    pub const KEY_F: u8 = 70;
    pub const KEY_G: u8 = 71;
    pub const KEY_H: u8 = 72;
    pub const KEY_I: u8 = 73;
    pub const KEY_J: u8 = 74;
    pub const KEY_K: u8 = 75;
    pub const KEY_L: u8 = 76;
    pub const KEY_M: u8 = 77;
    pub const KEY_N: u8 = 78;
    pub const KEY_O: u8 = 79;
    pub const KEY_P: u8 = 80;
    pub const KEY_Q: u8 = 81;
    pub const KEY_R: u8 = 82;
    pub const KEY_S: u8 = 83;
    pub const KEY_T: u8 = 84;
    pub const KEY_U: u8 = 85;
    pub const KEY_V: u8 = 86;
    pub const KEY_W: u8 = 87;
    pub const KEY_X: u8 = 88;
    pub const KEY_Y: u8 = 89;
    pub const KEY_Z: u8 = 90;

    // Numbers (top row)
    pub const KEY_0: u8 = 48;
    pub const KEY_1: u8 = 49;
    pub const KEY_2: u8 = 50;
    pub const KEY_3: u8 = 51;
    pub const KEY_4: u8 = 52;
    pub const KEY_5: u8 = 53;
    pub const KEY_6: u8 = 54;
    pub const KEY_7: u8 = 55;
    pub const KEY_8: u8 = 56;
    pub const KEY_9: u8 = 57;

    // Function keys
    pub const KEY_F1: u8 = 112;
    pub const KEY_F2: u8 = 113;
    pub const KEY_F3: u8 = 114;
    pub const KEY_F4: u8 = 115;
    pub const KEY_F5: u8 = 116;
    pub const KEY_F6: u8 = 117;
    pub const KEY_F7: u8 = 118;
    pub const KEY_F8: u8 = 119;
    pub const KEY_F9: u8 = 120;
    pub const KEY_F10: u8 = 121;
    pub const KEY_F11: u8 = 122;
    pub const KEY_F12: u8 = 123;

    // Control keys
    pub const KEY_BACKSPACE: u8 = 8;
    pub const KEY_TAB: u8 = 9;
    pub const KEY_ENTER: u8 = 13;
    pub const KEY_SHIFT: u8 = 16;
    pub const KEY_CTRL: u8 = 17;
    pub const KEY_ALT: u8 = 18;
    pub const KEY_PAUSE: u8 = 19;
    pub const KEY_CAPS_LOCK: u8 = 20;
    pub const KEY_ESCAPE: u8 = 27;
    pub const KEY_SPACE: u8 = 32;
    pub const KEY_PAGE_UP: u8 = 33;
    pub const KEY_PAGE_DOWN: u8 = 34;
    pub const KEY_END: u8 = 35;
    pub const KEY_HOME: u8 = 36;
    pub const KEY_LEFT: u8 = 37;
    pub const KEY_UP: u8 = 38;
    pub const KEY_RIGHT: u8 = 39;
    pub const KEY_DOWN: u8 = 40;
    pub const KEY_INSERT: u8 = 45;
    pub const KEY_DELETE: u8 = 46;

    // Punctuation
    pub const KEY_SEMICOLON: u8 = 186;
    pub const KEY_EQUAL: u8 = 187;
    pub const KEY_COMMA: u8 = 188;
    pub const KEY_MINUS: u8 = 189;
    pub const KEY_PERIOD: u8 = 190;
    pub const KEY_SLASH: u8 = 191;
    pub const KEY_GRAVE: u8 = 192;
    pub const KEY_LEFT_BRACKET: u8 = 219;
    pub const KEY_BACKSLASH: u8 = 220;
    pub const KEY_RIGHT_BRACKET: u8 = 221;
    pub const KEY_APOSTROPHE: u8 = 222;

    // Numpad
    pub const KEY_NUMPAD_0: u8 = 96;
    pub const KEY_NUMPAD_1: u8 = 97;
    pub const KEY_NUMPAD_2: u8 = 98;
    pub const KEY_NUMPAD_3: u8 = 99;
    pub const KEY_NUMPAD_4: u8 = 100;
    pub const KEY_NUMPAD_5: u8 = 101;
    pub const KEY_NUMPAD_6: u8 = 102;
    pub const KEY_NUMPAD_7: u8 = 103;
    pub const KEY_NUMPAD_8: u8 = 104;
    pub const KEY_NUMPAD_9: u8 = 105;
    pub const KEY_NUMPAD_MULTIPLY: u8 = 106;
    pub const KEY_NUMPAD_ADD: u8 = 107;
    pub const KEY_NUMPAD_SUBTRACT: u8 = 109;
    pub const KEY_NUMPAD_DECIMAL: u8 = 110;
    pub const KEY_NUMPAD_DIVIDE: u8 = 111;

    // Lock keys
    pub const KEY_NUM_LOCK: u8 = 144;
    pub const KEY_SCROLL_LOCK: u8 = 145;

    // Windows keys
    pub const KEY_META_LEFT: u8 = 91;
    pub const KEY_META_RIGHT: u8 = 92;
    pub const KEY_CONTEXT_MENU: u8 = 93;
}

/// JavaScript keyCode to USB HID keyCode mapping table
/// Using a fixed-size array for O(1) lookup instead of HashMap
/// Index = JavaScript keyCode, Value = USB HID keyCode (0 means unmapped)
static JS_TO_USB_TABLE: [u8; 256] = {
    let mut table = [0u8; 256];

    // Letters A-Z (JS 65-90 -> USB 0x04-0x1D)
    let mut i = 0u8;
    while i < 26 {
        table[(65 + i) as usize] = usb::KEY_A + i;
        i += 1;
    }

    // Numbers 1-9, 0 (JS 49-57, 48 -> USB 0x1E-0x27)
    table[49] = usb::KEY_1;  // 1
    table[50] = usb::KEY_2;  // 2
    table[51] = usb::KEY_3;  // 3
    table[52] = usb::KEY_4;  // 4
    table[53] = usb::KEY_5;  // 5
    table[54] = usb::KEY_6;  // 6
    table[55] = usb::KEY_7;  // 7
    table[56] = usb::KEY_8;  // 8
    table[57] = usb::KEY_9;  // 9
    table[48] = usb::KEY_0;  // 0

    // Function keys F1-F12 (JS 112-123 -> USB 0x3A-0x45)
    table[112] = usb::KEY_F1;
    table[113] = usb::KEY_F2;
    table[114] = usb::KEY_F3;
    table[115] = usb::KEY_F4;
    table[116] = usb::KEY_F5;
    table[117] = usb::KEY_F6;
    table[118] = usb::KEY_F7;
    table[119] = usb::KEY_F8;
    table[120] = usb::KEY_F9;
    table[121] = usb::KEY_F10;
    table[122] = usb::KEY_F11;
    table[123] = usb::KEY_F12;

    // Control keys
    table[13] = usb::KEY_ENTER;      // Enter
    table[27] = usb::KEY_ESCAPE;     // Escape
    table[8] = usb::KEY_BACKSPACE;   // Backspace
    table[9] = usb::KEY_TAB;         // Tab
    table[32] = usb::KEY_SPACE;      // Space
    table[20] = usb::KEY_CAPS_LOCK;  // Caps Lock

    // Punctuation (JS codes vary by browser/layout)
    table[189] = usb::KEY_MINUS;         // -
    table[187] = usb::KEY_EQUAL;         // =
    table[219] = usb::KEY_LEFT_BRACKET;  // [
    table[221] = usb::KEY_RIGHT_BRACKET; // ]
    table[220] = usb::KEY_BACKSLASH;     // \
    table[186] = usb::KEY_SEMICOLON;     // ;
    table[222] = usb::KEY_APOSTROPHE;    // '
    table[192] = usb::KEY_GRAVE;         // `
    table[188] = usb::KEY_COMMA;         // ,
    table[190] = usb::KEY_PERIOD;        // .
    table[191] = usb::KEY_SLASH;         // /

    // Navigation keys
    table[45] = usb::KEY_INSERT;
    table[46] = usb::KEY_DELETE;
    table[36] = usb::KEY_HOME;
    table[35] = usb::KEY_END;
    table[33] = usb::KEY_PAGE_UP;
    table[34] = usb::KEY_PAGE_DOWN;

    // Arrow keys
    table[39] = usb::KEY_RIGHT_ARROW;
    table[37] = usb::KEY_LEFT_ARROW;
    table[40] = usb::KEY_DOWN_ARROW;
    table[38] = usb::KEY_UP_ARROW;

    // Numpad
    table[144] = usb::KEY_NUM_LOCK;
    table[111] = usb::KEY_NUMPAD_DIVIDE;
    table[106] = usb::KEY_NUMPAD_MULTIPLY;
    table[109] = usb::KEY_NUMPAD_MINUS;
    table[107] = usb::KEY_NUMPAD_PLUS;
    table[96] = usb::KEY_NUMPAD_0;
    table[97] = usb::KEY_NUMPAD_1;
    table[98] = usb::KEY_NUMPAD_2;
    table[99] = usb::KEY_NUMPAD_3;
    table[100] = usb::KEY_NUMPAD_4;
    table[101] = usb::KEY_NUMPAD_5;
    table[102] = usb::KEY_NUMPAD_6;
    table[103] = usb::KEY_NUMPAD_7;
    table[104] = usb::KEY_NUMPAD_8;
    table[105] = usb::KEY_NUMPAD_9;
    table[110] = usb::KEY_NUMPAD_DECIMAL;

    // Special keys
    table[19] = usb::KEY_PAUSE;
    table[145] = usb::KEY_SCROLL_LOCK;
    table[93] = usb::KEY_APPLICATION;  // Context menu

    // Modifier keys
    table[17] = usb::KEY_LEFT_CTRL;
    table[16] = usb::KEY_LEFT_SHIFT;
    table[18] = usb::KEY_LEFT_ALT;
    table[91] = usb::KEY_LEFT_META;   // Left Windows/Command
    table[92] = usb::KEY_RIGHT_META;  // Right Windows/Command

    table
};

/// Convert JavaScript keyCode to USB HID keyCode
///
/// Uses a fixed-size lookup table for O(1) performance.
/// Returns None if the key code is not mapped.
#[inline]
pub fn js_to_usb(js_code: u8) -> Option<u8> {
    let usb_code = JS_TO_USB_TABLE[js_code as usize];
    if usb_code != 0 {
        Some(usb_code)
    } else {
        None
    }
}

/// Check if a key code is a modifier key
pub fn is_modifier_key(usb_code: u8) -> bool {
    (0xE0..=0xE7).contains(&usb_code)
}

/// Get modifier bit for a modifier key
pub fn modifier_bit(usb_code: u8) -> Option<u8> {
    match usb_code {
        usb::KEY_LEFT_CTRL => Some(0x01),
        usb::KEY_LEFT_SHIFT => Some(0x02),
        usb::KEY_LEFT_ALT => Some(0x04),
        usb::KEY_LEFT_META => Some(0x08),
        usb::KEY_RIGHT_CTRL => Some(0x10),
        usb::KEY_RIGHT_SHIFT => Some(0x20),
        usb::KEY_RIGHT_ALT => Some(0x40),
        usb::KEY_RIGHT_META => Some(0x80),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_letter_mapping() {
        assert_eq!(js_to_usb(65), Some(usb::KEY_A)); // A
        assert_eq!(js_to_usb(90), Some(usb::KEY_Z)); // Z
    }

    #[test]
    fn test_number_mapping() {
        assert_eq!(js_to_usb(48), Some(usb::KEY_0));
        assert_eq!(js_to_usb(49), Some(usb::KEY_1));
    }

    #[test]
    fn test_modifier_key() {
        assert!(is_modifier_key(usb::KEY_LEFT_CTRL));
        assert!(is_modifier_key(usb::KEY_RIGHT_SHIFT));
        assert!(!is_modifier_key(usb::KEY_A));
    }
}
