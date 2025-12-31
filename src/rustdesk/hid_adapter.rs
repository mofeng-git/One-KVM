//! RustDesk HID Adapter
//!
//! Converts RustDesk HID events (KeyEvent, MouseEvent) to One-KVM HID events.

use crate::hid::{
    KeyboardEvent, KeyboardModifiers, KeyEventType,
    MouseButton, MouseEvent as OneKvmMouseEvent, MouseEventType,
};
use super::protocol::hbb::{self, ControlKey, KeyEvent, MouseEvent};

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
            // Pure move event
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

            // For wheel events, button_id indicates scroll direction
            // Positive = scroll up, Negative = scroll down
            // The actual scroll amount may be encoded differently
            let scroll = if button_id > 0 { 1i8 } else { -1i8 };
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
pub fn convert_key_event(event: &KeyEvent) -> Option<KeyboardEvent> {
    let pressed = event.down || event.press;
    let event_type = if pressed { KeyEventType::Down } else { KeyEventType::Up };

    // Parse modifiers from the event
    let modifiers = parse_modifiers(event);

    // Handle control keys
    if let Some(hbb::key_event::Union::ControlKey(ck)) = &event.union {
        if let Some(key) = control_key_to_hid(*ck) {
            return Some(KeyboardEvent {
                event_type,
                key,
                modifiers,
            });
        }
    }

    // Handle character keys (chr field contains platform-specific keycode)
    if let Some(hbb::key_event::Union::Chr(chr)) = &event.union {
        // chr contains USB HID scancode on Windows, X11 keycode on Linux
        if let Some(key) = keycode_to_hid(*chr) {
            return Some(KeyboardEvent {
                event_type,
                key,
                modifiers,
            });
        }
    }

    // Handle unicode (for text input, we'd need to convert to scancodes)
    // Unicode input requires more complex handling, skip for now

    None
}

/// Parse modifier keys from RustDesk KeyEvent into KeyboardModifiers
fn parse_modifiers(event: &KeyEvent) -> KeyboardModifiers {
    let mut modifiers = KeyboardModifiers::default();

    for modifier in &event.modifiers {
        match *modifier {
            x if x == ControlKey::Control as i32 => modifiers.left_ctrl = true,
            x if x == ControlKey::Shift as i32 => modifiers.left_shift = true,
            x if x == ControlKey::Alt as i32 => modifiers.left_alt = true,
            x if x == ControlKey::Meta as i32 => modifiers.left_meta = true,
            x if x == ControlKey::RControl as i32 => modifiers.right_ctrl = true,
            x if x == ControlKey::RShift as i32 => modifiers.right_shift = true,
            x if x == ControlKey::RAlt as i32 => modifiers.right_alt = true,
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
/// This is a simplified mapping for X11 keycodes (Linux)
fn keycode_to_hid(keycode: u32) -> Option<u8> {
    match keycode {
        // Numbers 1-9 then 0 (X11 keycodes 10-19)
        10 => Some(0x27), // 0
        11..=19 => Some((keycode - 11 + 0x1E) as u8), // 1-9

        // Punctuation before letters block
        20 => Some(0x2D), // -
        21 => Some(0x2E), // =
        34 => Some(0x2F), // [
        35 => Some(0x30), // ]

        // Letters A-Z (X11 keycodes 38-63 map to various letters, not strictly A-Z)
        // Note: X11 keycodes are row-based, not alphabetical
        // Row 1: q(24) w(25) e(26) r(27) t(28) y(29) u(30) i(31) o(32) p(33)
        // Row 2: a(38) s(39) d(40) f(41) g(42) h(43) j(44) k(45) l(46)
        // Row 3: z(52) x(53) c(54) v(55) b(56) n(57) m(58)
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
        38 => Some(0x04), // a
        39 => Some(0x16), // s
        40 => Some(0x07), // d
        41 => Some(0x09), // f
        42 => Some(0x0A), // g
        43 => Some(0x0B), // h
        44 => Some(0x0D), // j
        45 => Some(0x0E), // k
        46 => Some(0x0F), // l
        47 => Some(0x33), // ; (semicolon)
        48 => Some(0x34), // ' (apostrophe)
        49 => Some(0x35), // ` (grave)
        51 => Some(0x31), // \ (backslash)
        52 => Some(0x1D), // z
        53 => Some(0x1B), // x
        54 => Some(0x06), // c
        55 => Some(0x19), // v
        56 => Some(0x05), // b
        57 => Some(0x11), // n
        58 => Some(0x10), // m
        59 => Some(0x36), // , (comma)
        60 => Some(0x37), // . (period)
        61 => Some(0x38), // / (slash)

        // Space
        65 => Some(0x2C),

        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_mouse_buttons() {
        let buttons = parse_mouse_buttons(mouse_mask::LEFT | mouse_mask::RIGHT);
        assert!(buttons.contains(&MouseButton::Left));
        assert!(buttons.contains(&MouseButton::Right));
        assert!(!buttons.contains(&MouseButton::Middle));
    }

    #[test]
    fn test_parse_scroll() {
        assert_eq!(parse_scroll(mouse_mask::SCROLL_UP), 1);
        assert_eq!(parse_scroll(mouse_mask::SCROLL_DOWN), -1);
        assert_eq!(parse_scroll(0), 0);
    }

    #[test]
    fn test_control_key_mapping() {
        assert_eq!(control_key_to_hid(ControlKey::Escape as i32), Some(0x29));
        assert_eq!(control_key_to_hid(ControlKey::Return as i32), Some(0x28));
        assert_eq!(control_key_to_hid(ControlKey::Space as i32), Some(0x2C));
    }

    #[test]
    fn test_convert_mouse_event() {
        let rustdesk_event = MouseEvent {
            x: 500,
            y: 300,
            mask: mouse_mask::LEFT,
            ..Default::default()
        };

        let events = convert_mouse_event(&rustdesk_event, 1920, 1080);
        assert!(!events.is_empty());

        // First event should be MoveAbs
        assert_eq!(events[0].event_type, MouseEventType::MoveAbs);

        // Should have a button down event
        assert!(events.iter().any(|e| e.event_type == MouseEventType::Down && e.button == Some(MouseButton::Left)));
    }

    #[test]
    fn test_convert_key_event() {
        let key_event = KeyEvent {
            down: true,
            press: false,
            union: Some(hbb::key_event::Union::ControlKey(ControlKey::Return as i32)),
            ..Default::default()
        };

        let result = convert_key_event(&key_event);
        assert!(result.is_some());

        let kb_event = result.unwrap();
        assert_eq!(kb_event.event_type, KeyEventType::Down);
        assert_eq!(kb_event.key, 0x28); // Return key USB HID code
    }
}
