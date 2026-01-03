//! RustDesk HID Adapter
//!
//! Converts RustDesk HID events (KeyEvent, MouseEvent) to One-KVM HID events.

use protobuf::Enum;
use crate::hid::{
    KeyboardEvent, KeyboardModifiers, KeyEventType,
    MouseButton, MouseEvent as OneKvmMouseEvent, MouseEventType,
};
use super::protocol::{KeyEvent, MouseEvent, ControlKey};
use super::protocol::hbb::message::key_event as ke_union;

/// Mouse event types from RustDesk protocol
/// mask = (button << 3) | event_type
pub mod mouse_type {
    pub const MOVE: i32 = 0;
    pub const DOWN: i32 = 1;
    pub const UP: i32 = 2;
    pub const WHEEL: i32 = 3;
    pub const TRACKPAD: i32 = 4;
}

/// Mouse button IDs from RustDesk protocol (before left shift by 3)
pub mod mouse_button {
    pub const LEFT: i32 = 0x01;
    pub const RIGHT: i32 = 0x02;
    pub const WHEEL: i32 = 0x04;
    pub const BACK: i32 = 0x08;
    pub const FORWARD: i32 = 0x10;
}

/// Convert RustDesk MouseEvent to One-KVM MouseEvent(s)
/// Returns a Vec because a single RustDesk event may need multiple One-KVM events
/// (e.g., move + button + scroll)
pub fn convert_mouse_event(event: &MouseEvent, screen_width: u32, screen_height: u32) -> Vec<OneKvmMouseEvent> {
    let mut events = Vec::new();

    // RustDesk uses absolute coordinates
    let x = event.x.max(0) as u32;
    let y = event.y.max(0) as u32;

    // Normalize to 0-32767 range for absolute mouse (USB HID standard)
    let abs_x = ((x as u64 * 32767) / screen_width.max(1) as u64) as i32;
    let abs_y = ((y as u64 * 32767) / screen_height.max(1) as u64) as i32;

    // Parse RustDesk mask format: (button << 3) | event_type
    let event_type = event.mask & 0x07;
    let button_id = event.mask >> 3;

    match event_type {
        mouse_type::MOVE => {
            // Move event - may have button held down (button_id > 0 means dragging)
            // Just send move, button state is tracked separately by HID backend
            events.push(OneKvmMouseEvent {
                event_type: MouseEventType::MoveAbs,
                x: abs_x,
                y: abs_y,
                button: None,
                scroll: 0,
            });
        }
        mouse_type::DOWN => {
            // Button down - first move, then press
            events.push(OneKvmMouseEvent {
                event_type: MouseEventType::MoveAbs,
                x: abs_x,
                y: abs_y,
                button: None,
                scroll: 0,
            });

            if let Some(button) = button_id_to_button(button_id) {
                events.push(OneKvmMouseEvent {
                    event_type: MouseEventType::Down,
                    x: abs_x,
                    y: abs_y,
                    button: Some(button),
                    scroll: 0,
                });
            }
        }
        mouse_type::UP => {
            // Button up - first move, then release
            events.push(OneKvmMouseEvent {
                event_type: MouseEventType::MoveAbs,
                x: abs_x,
                y: abs_y,
                button: None,
                scroll: 0,
            });

            if let Some(button) = button_id_to_button(button_id) {
                events.push(OneKvmMouseEvent {
                    event_type: MouseEventType::Up,
                    x: abs_x,
                    y: abs_y,
                    button: Some(button),
                    scroll: 0,
                });
            }
        }
        mouse_type::WHEEL => {
            // Scroll event - move first, then scroll
            events.push(OneKvmMouseEvent {
                event_type: MouseEventType::MoveAbs,
                x: abs_x,
                y: abs_y,
                button: None,
                scroll: 0,
            });

            // RustDesk encodes scroll direction in the y coordinate
            // Positive y = scroll up, Negative y = scroll down
            // The button_id field is not used for direction
            let scroll = if event.y > 0 { 1i8 } else { -1i8 };
            events.push(OneKvmMouseEvent {
                event_type: MouseEventType::Scroll,
                x: abs_x,
                y: abs_y,
                button: None,
                scroll,
            });
        }
        _ => {
            // Unknown event type, just move
            events.push(OneKvmMouseEvent {
                event_type: MouseEventType::MoveAbs,
                x: abs_x,
                y: abs_y,
                button: None,
                scroll: 0,
            });
        }
    }

    events
}

/// Convert RustDesk button ID to One-KVM MouseButton
fn button_id_to_button(button_id: i32) -> Option<MouseButton> {
    match button_id {
        mouse_button::LEFT => Some(MouseButton::Left),
        mouse_button::RIGHT => Some(MouseButton::Right),
        mouse_button::WHEEL => Some(MouseButton::Middle),
        _ => None,
    }
}

/// Convert RustDesk KeyEvent to One-KVM KeyboardEvent
///
/// RustDesk KeyEvent has two modes:
/// - down=true/false: Key state (pressed/released)
/// - press=true: Complete key press (down + up), used for typing
///
/// For press=true events, we only send Down and let the caller handle
/// the timing for Up event if needed. Most systems handle this correctly.
pub fn convert_key_event(event: &KeyEvent) -> Option<KeyboardEvent> {
    // Determine if this is a key down or key up event
    // press=true means "key was pressed" (down event)
    // down=true means key is currently held down
    // down=false with press=false means key was released
    let event_type = if event.down || event.press {
        KeyEventType::Down
    } else {
        KeyEventType::Up
    };

    // For modifier keys sent as ControlKey, don't include them in modifiers
    // to avoid double-pressing. The modifier will be tracked by HID state.
    let modifiers = if is_modifier_control_key(event) {
        KeyboardModifiers::default()
    } else {
        parse_modifiers(event)
    };

    // Handle control keys
    if let Some(ke_union::Union::ControlKey(ck)) = &event.union {
        if let Some(key) = control_key_to_hid(ck.value()) {
            return Some(KeyboardEvent {
                event_type,
                key,
                modifiers,
                is_usb_hid: true, // Already converted to USB HID code
            });
        }
    }

    // Handle character keys (chr field contains platform-specific keycode)
    if let Some(ke_union::Union::Chr(chr)) = &event.union {
        // chr contains USB HID scancode on Windows, X11 keycode on Linux
        if let Some(key) = keycode_to_hid(*chr) {
            return Some(KeyboardEvent {
                event_type,
                key,
                modifiers,
                is_usb_hid: true, // Already converted to USB HID code
            });
        }
    }

    // Handle unicode (for text input, we'd need to convert to scancodes)
    // Unicode input requires more complex handling, skip for now

    None
}

/// Check if the event is a modifier key sent as ControlKey
fn is_modifier_control_key(event: &KeyEvent) -> bool {
    if let Some(ke_union::Union::ControlKey(ck)) = &event.union {
        let val = ck.value();
        return val == ControlKey::Control.value()
            || val == ControlKey::Shift.value()
            || val == ControlKey::Alt.value()
            || val == ControlKey::Meta.value()
            || val == ControlKey::RControl.value()
            || val == ControlKey::RShift.value()
            || val == ControlKey::RAlt.value();
    }
    false
}

/// Parse modifier keys from RustDesk KeyEvent into KeyboardModifiers
fn parse_modifiers(event: &KeyEvent) -> KeyboardModifiers {
    let mut modifiers = KeyboardModifiers::default();

    for modifier in &event.modifiers {
        let val = modifier.value();
        match val {
            x if x == ControlKey::Control.value() => modifiers.left_ctrl = true,
            x if x == ControlKey::Shift.value() => modifiers.left_shift = true,
            x if x == ControlKey::Alt.value() => modifiers.left_alt = true,
            x if x == ControlKey::Meta.value() => modifiers.left_meta = true,
            x if x == ControlKey::RControl.value() => modifiers.right_ctrl = true,
            x if x == ControlKey::RShift.value() => modifiers.right_shift = true,
            x if x == ControlKey::RAlt.value() => modifiers.right_alt = true,
            _ => {}
        }
    }

    modifiers
}

/// Convert RustDesk ControlKey to USB HID usage code
fn control_key_to_hid(key: i32) -> Option<u8> {
    match key {
        x if x == ControlKey::Alt as i32 => Some(0xE2),      // Left Alt
        x if x == ControlKey::Backspace as i32 => Some(0x2A),
        x if x == ControlKey::CapsLock as i32 => Some(0x39),
        x if x == ControlKey::Control as i32 => Some(0xE0),  // Left Ctrl
        x if x == ControlKey::Delete as i32 => Some(0x4C),
        x if x == ControlKey::DownArrow as i32 => Some(0x51),
        x if x == ControlKey::End as i32 => Some(0x4D),
        x if x == ControlKey::Escape as i32 => Some(0x29),
        x if x == ControlKey::F1 as i32 => Some(0x3A),
        x if x == ControlKey::F2 as i32 => Some(0x3B),
        x if x == ControlKey::F3 as i32 => Some(0x3C),
        x if x == ControlKey::F4 as i32 => Some(0x3D),
        x if x == ControlKey::F5 as i32 => Some(0x3E),
        x if x == ControlKey::F6 as i32 => Some(0x3F),
        x if x == ControlKey::F7 as i32 => Some(0x40),
        x if x == ControlKey::F8 as i32 => Some(0x41),
        x if x == ControlKey::F9 as i32 => Some(0x42),
        x if x == ControlKey::F10 as i32 => Some(0x43),
        x if x == ControlKey::F11 as i32 => Some(0x44),
        x if x == ControlKey::F12 as i32 => Some(0x45),
        x if x == ControlKey::Home as i32 => Some(0x4A),
        x if x == ControlKey::LeftArrow as i32 => Some(0x50),
        x if x == ControlKey::Meta as i32 => Some(0xE3),     // Left GUI/Windows
        x if x == ControlKey::PageDown as i32 => Some(0x4E),
        x if x == ControlKey::PageUp as i32 => Some(0x4B),
        x if x == ControlKey::Return as i32 => Some(0x28),
        x if x == ControlKey::RightArrow as i32 => Some(0x4F),
        x if x == ControlKey::Shift as i32 => Some(0xE1),    // Left Shift
        x if x == ControlKey::Space as i32 => Some(0x2C),
        x if x == ControlKey::Tab as i32 => Some(0x2B),
        x if x == ControlKey::UpArrow as i32 => Some(0x52),
        x if x == ControlKey::Numpad0 as i32 => Some(0x62),
        x if x == ControlKey::Numpad1 as i32 => Some(0x59),
        x if x == ControlKey::Numpad2 as i32 => Some(0x5A),
        x if x == ControlKey::Numpad3 as i32 => Some(0x5B),
        x if x == ControlKey::Numpad4 as i32 => Some(0x5C),
        x if x == ControlKey::Numpad5 as i32 => Some(0x5D),
        x if x == ControlKey::Numpad6 as i32 => Some(0x5E),
        x if x == ControlKey::Numpad7 as i32 => Some(0x5F),
        x if x == ControlKey::Numpad8 as i32 => Some(0x60),
        x if x == ControlKey::Numpad9 as i32 => Some(0x61),
        x if x == ControlKey::Insert as i32 => Some(0x49),
        x if x == ControlKey::Pause as i32 => Some(0x48),
        x if x == ControlKey::Scroll as i32 => Some(0x47),
        x if x == ControlKey::NumLock as i32 => Some(0x53),
        x if x == ControlKey::RShift as i32 => Some(0xE5),
        x if x == ControlKey::RControl as i32 => Some(0xE4),
        x if x == ControlKey::RAlt as i32 => Some(0xE6),
        x if x == ControlKey::Multiply as i32 => Some(0x55),
        x if x == ControlKey::Add as i32 => Some(0x57),
        x if x == ControlKey::Subtract as i32 => Some(0x56),
        x if x == ControlKey::Decimal as i32 => Some(0x63),
        x if x == ControlKey::Divide as i32 => Some(0x54),
        x if x == ControlKey::NumpadEnter as i32 => Some(0x58),
        _ => None,
    }
}

/// Convert platform keycode to USB HID usage code
/// Handles Windows Virtual Key Codes, X11 keycodes, and ASCII codes
fn keycode_to_hid(keycode: u32) -> Option<u8> {
    // First try ASCII code mapping (RustDesk often sends ASCII codes)
    if let Some(hid) = ascii_to_hid(keycode) {
        return Some(hid);
    }
    // Then try Windows Virtual Key Code mapping
    if let Some(hid) = windows_vk_to_hid(keycode) {
        return Some(hid);
    }
    // Fall back to X11 keycode mapping for Linux clients
    x11_keycode_to_hid(keycode)
}

/// Convert ASCII code to USB HID usage code
fn ascii_to_hid(ascii: u32) -> Option<u8> {
    match ascii {
        // Lowercase letters a-z (ASCII 97-122)
        97..=122 => {
            // USB HID: a=0x04, b=0x05, ..., z=0x1D
            Some((ascii - 97 + 0x04) as u8)
        }
        // Uppercase letters A-Z (ASCII 65-90)
        65..=90 => {
            // USB HID: A=0x04, B=0x05, ..., Z=0x1D (same as lowercase)
            Some((ascii - 65 + 0x04) as u8)
        }
        // Numbers 0-9 (ASCII 48-57)
        48 => Some(0x27), // 0
        49..=57 => Some((ascii - 49 + 0x1E) as u8), // 1-9
        // Common punctuation
        32 => Some(0x2C),  // Space
        13 => Some(0x28),  // Enter (CR)
        10 => Some(0x28),  // Enter (LF)
        9 => Some(0x2B),   // Tab
        27 => Some(0x29),  // Escape
        8 => Some(0x2A),   // Backspace
        127 => Some(0x4C), // Delete
        // Symbols (US keyboard layout)
        45 => Some(0x2D),  // -
        61 => Some(0x2E),  // =
        91 => Some(0x2F),  // [
        93 => Some(0x30),  // ]
        92 => Some(0x31),  // \
        59 => Some(0x33),  // ;
        39 => Some(0x34),  // '
        96 => Some(0x35),  // `
        44 => Some(0x36),  // ,
        46 => Some(0x37),  // .
        47 => Some(0x38),  // /
        _ => None,
    }
}

/// Convert Windows Virtual Key Code to USB HID usage code
fn windows_vk_to_hid(vk: u32) -> Option<u8> {
    match vk {
        // Letters A-Z (VK_A=0x41 to VK_Z=0x5A)
        0x41..=0x5A => {
            // USB HID: A=0x04, B=0x05, ..., Z=0x1D
            let letter = (vk - 0x41) as u8;
            Some(match letter {
                0 => 0x04,  // A
                1 => 0x05,  // B
                2 => 0x06,  // C
                3 => 0x07,  // D
                4 => 0x08,  // E
                5 => 0x09,  // F
                6 => 0x0A,  // G
                7 => 0x0B,  // H
                8 => 0x0C,  // I
                9 => 0x0D,  // J
                10 => 0x0E, // K
                11 => 0x0F, // L
                12 => 0x10, // M
                13 => 0x11, // N
                14 => 0x12, // O
                15 => 0x13, // P
                16 => 0x14, // Q
                17 => 0x15, // R
                18 => 0x16, // S
                19 => 0x17, // T
                20 => 0x18, // U
                21 => 0x19, // V
                22 => 0x1A, // W
                23 => 0x1B, // X
                24 => 0x1C, // Y
                25 => 0x1D, // Z
                _ => return None,
            })
        }
        // Numbers 0-9 (VK_0=0x30 to VK_9=0x39)
        0x30 => Some(0x27), // 0
        0x31..=0x39 => Some((vk - 0x31 + 0x1E) as u8), // 1-9
        // Numpad 0-9 (VK_NUMPAD0=0x60 to VK_NUMPAD9=0x69)
        0x60 => Some(0x62), // Numpad 0
        0x61..=0x69 => Some((vk - 0x61 + 0x59) as u8), // Numpad 1-9
        // Numpad operators
        0x6A => Some(0x55), // Numpad *
        0x6B => Some(0x57), // Numpad +
        0x6D => Some(0x56), // Numpad -
        0x6E => Some(0x63), // Numpad .
        0x6F => Some(0x54), // Numpad /
        // Function keys F1-F12 (VK_F1=0x70 to VK_F12=0x7B)
        0x70..=0x7B => Some((vk - 0x70 + 0x3A) as u8),
        // Special keys
        0x08 => Some(0x2A), // Backspace
        0x09 => Some(0x2B), // Tab
        0x0D => Some(0x28), // Enter
        0x1B => Some(0x29), // Escape
        0x20 => Some(0x2C), // Space
        0x21 => Some(0x4B), // Page Up
        0x22 => Some(0x4E), // Page Down
        0x23 => Some(0x4D), // End
        0x24 => Some(0x4A), // Home
        0x25 => Some(0x50), // Left Arrow
        0x26 => Some(0x52), // Up Arrow
        0x27 => Some(0x4F), // Right Arrow
        0x28 => Some(0x51), // Down Arrow
        0x2D => Some(0x49), // Insert
        0x2E => Some(0x4C), // Delete
        // OEM keys (US keyboard layout)
        0xBA => Some(0x33), // ; :
        0xBB => Some(0x2E), // = +
        0xBC => Some(0x36), // , <
        0xBD => Some(0x2D), // - _
        0xBE => Some(0x37), // . >
        0xBF => Some(0x38), // / ?
        0xC0 => Some(0x35), // ` ~
        0xDB => Some(0x2F), // [ {
        0xDC => Some(0x31), // \ |
        0xDD => Some(0x30), // ] }
        0xDE => Some(0x34), // ' "
        // Lock keys
        0x14 => Some(0x39), // Caps Lock
        0x90 => Some(0x53), // Num Lock
        0x91 => Some(0x47), // Scroll Lock
        // Print Screen, Pause
        0x2C => Some(0x46), // Print Screen
        0x13 => Some(0x48), // Pause
        _ => None,
    }
}

/// Convert X11 keycode to USB HID usage code (for Linux clients)
fn x11_keycode_to_hid(keycode: u32) -> Option<u8> {
    match keycode {
        // Numbers: X11 keycode 10="1", 11="2", ..., 18="9", 19="0"
        10..=18 => Some((keycode - 10 + 0x1E) as u8), // 1-9
        19 => Some(0x27), // 0
        // Punctuation
        20 => Some(0x2D), // -
        21 => Some(0x2E), // =
        34 => Some(0x2F), // [
        35 => Some(0x30), // ]
        // Letters (X11 keycodes are row-based)
        // Row 1: q(24) w(25) e(26) r(27) t(28) y(29) u(30) i(31) o(32) p(33)
        24 => Some(0x14), // q
        25 => Some(0x1A), // w
        26 => Some(0x08), // e
        27 => Some(0x15), // r
        28 => Some(0x17), // t
        29 => Some(0x1C), // y
        30 => Some(0x18), // u
        31 => Some(0x0C), // i
        32 => Some(0x12), // o
        33 => Some(0x13), // p
        // Row 2: a(38) s(39) d(40) f(41) g(42) h(43) j(44) k(45) l(46)
        38 => Some(0x04), // a
        39 => Some(0x16), // s
        40 => Some(0x07), // d
        41 => Some(0x09), // f
        42 => Some(0x0A), // g
        43 => Some(0x0B), // h
        44 => Some(0x0D), // j
        45 => Some(0x0E), // k
        46 => Some(0x0F), // l
        47 => Some(0x33), // ;
        48 => Some(0x34), // '
        49 => Some(0x35), // `
        51 => Some(0x31), // \
        // Row 3: z(52) x(53) c(54) v(55) b(56) n(57) m(58)
        52 => Some(0x1D), // z
        53 => Some(0x1B), // x
        54 => Some(0x06), // c
        55 => Some(0x19), // v
        56 => Some(0x05), // b
        57 => Some(0x11), // n
        58 => Some(0x10), // m
        59 => Some(0x36), // ,
        60 => Some(0x37), // .
        61 => Some(0x38), // /
        // Space
        65 => Some(0x2C),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_control_key_mapping() {
        assert_eq!(control_key_to_hid(ControlKey::Escape.value()), Some(0x29));
        assert_eq!(control_key_to_hid(ControlKey::Return.value()), Some(0x28));
        assert_eq!(control_key_to_hid(ControlKey::Space.value()), Some(0x2C));
    }

    #[test]
    fn test_convert_mouse_move() {
        let mut event = MouseEvent::new();
        event.x = 500;
        event.y = 300;
        event.mask = mouse_type::MOVE; // Pure move event

        let events = convert_mouse_event(&event, 1920, 1080);
        assert!(!events.is_empty());
        assert_eq!(events[0].event_type, MouseEventType::MoveAbs);
    }

    #[test]
    fn test_convert_mouse_button_down() {
        let mut event = MouseEvent::new();
        event.x = 500;
        event.y = 300;
        event.mask = (mouse_button::LEFT << 3) | mouse_type::DOWN;

        let events = convert_mouse_event(&event, 1920, 1080);
        assert!(events.len() >= 2);
        // Should have a button down event
        assert!(events.iter().any(|e| e.event_type == MouseEventType::Down && e.button == Some(MouseButton::Left)));
    }

    #[test]
    fn test_convert_key_event() {
        use protobuf::EnumOrUnknown;
        let mut key_event = KeyEvent::new();
        key_event.down = true;
        key_event.press = false;
        key_event.union = Some(ke_union::Union::ControlKey(EnumOrUnknown::new(ControlKey::Return)));

        let result = convert_key_event(&key_event);
        assert!(result.is_some());

        let kb_event = result.unwrap();
        assert_eq!(kb_event.event_type, KeyEventType::Down);
        assert_eq!(kb_event.key, 0x28); // Return key USB HID code
    }
}
