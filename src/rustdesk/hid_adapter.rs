use super::protocol::hbb::message::key_event as ke_union;
use super::protocol::{ControlKey, KeyEvent, KeyboardMode, MouseEvent};
use crate::hid::{
    CanonicalKey, KeyEventType, KeyboardEvent, KeyboardModifiers, MouseButton,
    MouseEvent as OneKvmMouseEvent, MouseEventType,
};
use protobuf::Enum;

pub mod mouse_type {
    pub const MOVE: i32 = 0;
    pub const DOWN: i32 = 1;
    pub const UP: i32 = 2;
    pub const WHEEL: i32 = 3;
    pub const TRACKPAD: i32 = 4;
    pub const MOVE_RELATIVE: i32 = 5;
}

pub mod mouse_button {
    pub const LEFT: i32 = 0x01;
    pub const RIGHT: i32 = 0x02;
    pub const WHEEL: i32 = 0x04;
    pub const BACK: i32 = 0x08;
    pub const FORWARD: i32 = 0x10;
}

pub fn convert_mouse_event(
    event: &MouseEvent,
    screen_width: u32,
    screen_height: u32,
) -> Vec<OneKvmMouseEvent> {
    let mut events = Vec::new();

    let event_type = event.mask & 0x07;
    let button_id = event.mask >> 3;

    match event_type {
        mouse_type::MOVE => {
            let x = event.x.max(0) as u32;
            let y = event.y.max(0) as u32;

            let abs_x = ((x as u64 * 32767) / screen_width.max(1) as u64) as i32;
            let abs_y = ((y as u64 * 32767) / screen_height.max(1) as u64) as i32;

            events.push(OneKvmMouseEvent {
                event_type: MouseEventType::MoveAbs,
                x: abs_x,
                y: abs_y,
                button: None,
                scroll: 0,
            });
        }
        mouse_type::MOVE_RELATIVE => {
            events.push(OneKvmMouseEvent {
                event_type: MouseEventType::Move,
                x: event.x,
                y: event.y,
                button: None,
                scroll: 0,
            });
        }
        mouse_type::DOWN => {
            if let Some(button) = button_id_to_button(button_id) {
                events.push(OneKvmMouseEvent {
                    event_type: MouseEventType::Down,
                    x: 0,
                    y: 0,
                    button: Some(button),
                    scroll: 0,
                });
            }
        }
        mouse_type::UP => {
            if let Some(button) = button_id_to_button(button_id) {
                events.push(OneKvmMouseEvent {
                    event_type: MouseEventType::Up,
                    x: 0,
                    y: 0,
                    button: Some(button),
                    scroll: 0,
                });
            }
        }
        mouse_type::WHEEL => {
            let scroll = if event.y > 0 { 1i8 } else { -1i8 };
            events.push(OneKvmMouseEvent {
                event_type: MouseEventType::Scroll,
                x: 0,
                y: 0,
                button: None,
                scroll,
            });
        }
        _ => {}
    }

    events
}

fn button_id_to_button(button_id: i32) -> Option<MouseButton> {
    match button_id {
        mouse_button::LEFT => Some(MouseButton::Left),
        mouse_button::RIGHT => Some(MouseButton::Right),
        mouse_button::WHEEL => Some(MouseButton::Middle),
        _ => None,
    }
}

pub fn convert_key_events(event: &KeyEvent) -> Vec<KeyboardEvent> {
    let base_modifiers = if is_modifier_control_key(event) {
        KeyboardModifiers::default()
    } else {
        parse_modifiers(event)
    };

    let Some(mapping) = key_event_to_hid(event, base_modifiers) else {
        return Vec::new();
    };

    if event.press {
        let up_modifiers = if mapping.added_shift {
            base_modifiers
        } else {
            mapping.modifiers
        };
        vec![
            KeyboardEvent {
                event_type: KeyEventType::Down,
                key: mapping.key,
                modifiers: mapping.modifiers,
            },
            KeyboardEvent {
                event_type: KeyEventType::Up,
                key: mapping.key,
                modifiers: up_modifiers,
            },
        ]
    } else {
        let event_type = if event.down {
            KeyEventType::Down
        } else {
            KeyEventType::Up
        };
        vec![KeyboardEvent {
            event_type,
            key: mapping.key,
            modifiers: mapping.modifiers,
        }]
    }
}

pub fn convert_key_event(event: &KeyEvent) -> Option<KeyboardEvent> {
    convert_key_events(event).into_iter().next()
}

#[derive(Debug, Clone, Copy)]
struct KeyMapping {
    key: CanonicalKey,
    modifiers: KeyboardModifiers,
    added_shift: bool,
}

fn key_event_to_hid(event: &KeyEvent, modifiers: KeyboardModifiers) -> Option<KeyMapping> {
    match &event.union {
        Some(ke_union::Union::ControlKey(ck)) => {
            let key = CanonicalKey::from_hid_usage(control_key_to_hid(ck.value())?)?;
            Some(KeyMapping {
                key,
                modifiers,
                added_shift: false,
            })
        }
        Some(ke_union::Union::Chr(chr)) => {
            if event.mode.value() != KeyboardMode::Map.value() {
                if let Some(mapping) = shifted_printable_char_to_hid(*chr, modifiers) {
                    return Some(mapping);
                }
            }
            let key = CanonicalKey::from_hid_usage(keycode_to_hid(*chr)?)?;
            Some(KeyMapping {
                key,
                modifiers,
                added_shift: false,
            })
        }
        Some(ke_union::Union::Unicode(unicode)) => {
            let mapping = printable_char_to_hid(*unicode, modifiers)?;
            Some(mapping)
        }
        _ => None,
    }
}

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

fn with_shift(mut modifiers: KeyboardModifiers) -> KeyboardModifiers {
    modifiers.left_shift = true;
    modifiers
}

fn shifted_mapping(key: CanonicalKey, modifiers: KeyboardModifiers) -> KeyMapping {
    let added_shift = !modifiers.left_shift && !modifiers.right_shift;
    KeyMapping {
        key,
        modifiers: with_shift(modifiers),
        added_shift,
    }
}

fn plain_mapping(key: CanonicalKey, modifiers: KeyboardModifiers) -> KeyMapping {
    KeyMapping {
        key,
        modifiers,
        added_shift: false,
    }
}

fn shifted_printable_char_to_hid(ch: u32, modifiers: KeyboardModifiers) -> Option<KeyMapping> {
    match ch {
        33 => Some(shifted_mapping(CanonicalKey::Digit1, modifiers)),
        64 => Some(shifted_mapping(CanonicalKey::Digit2, modifiers)),
        35 => Some(shifted_mapping(CanonicalKey::Digit3, modifiers)),
        36 => Some(shifted_mapping(CanonicalKey::Digit4, modifiers)),
        37 => Some(shifted_mapping(CanonicalKey::Digit5, modifiers)),
        94 => Some(shifted_mapping(CanonicalKey::Digit6, modifiers)),
        38 => Some(shifted_mapping(CanonicalKey::Digit7, modifiers)),
        42 => Some(shifted_mapping(CanonicalKey::Digit8, modifiers)),
        40 => Some(shifted_mapping(CanonicalKey::Digit9, modifiers)),
        41 => Some(shifted_mapping(CanonicalKey::Digit0, modifiers)),
        95 => Some(shifted_mapping(CanonicalKey::Minus, modifiers)),
        43 => Some(shifted_mapping(CanonicalKey::Equal, modifiers)),
        123 => Some(shifted_mapping(CanonicalKey::BracketLeft, modifiers)),
        125 => Some(shifted_mapping(CanonicalKey::BracketRight, modifiers)),
        124 => Some(shifted_mapping(CanonicalKey::Backslash, modifiers)),
        58 => Some(shifted_mapping(CanonicalKey::Semicolon, modifiers)),
        34 => Some(shifted_mapping(CanonicalKey::Quote, modifiers)),
        126 => Some(shifted_mapping(CanonicalKey::Backquote, modifiers)),
        60 => Some(shifted_mapping(CanonicalKey::Comma, modifiers)),
        62 => Some(shifted_mapping(CanonicalKey::Period, modifiers)),
        63 => Some(shifted_mapping(CanonicalKey::Slash, modifiers)),
        _ => None,
    }
}

fn printable_char_to_hid(ch: u32, modifiers: KeyboardModifiers) -> Option<KeyMapping> {
    match ch {
        65..=90 => Some(shifted_mapping(
            CanonicalKey::from_hid_usage((ch - 65 + 0x04) as u8)?,
            modifiers,
        )),
        97..=122 => Some(plain_mapping(
            CanonicalKey::from_hid_usage((ch - 97 + 0x04) as u8)?,
            modifiers,
        )),
        48 => Some(plain_mapping(CanonicalKey::Digit0, modifiers)),
        49 => Some(plain_mapping(CanonicalKey::Digit1, modifiers)),
        50 => Some(plain_mapping(CanonicalKey::Digit2, modifiers)),
        51 => Some(plain_mapping(CanonicalKey::Digit3, modifiers)),
        52 => Some(plain_mapping(CanonicalKey::Digit4, modifiers)),
        53 => Some(plain_mapping(CanonicalKey::Digit5, modifiers)),
        54 => Some(plain_mapping(CanonicalKey::Digit6, modifiers)),
        55 => Some(plain_mapping(CanonicalKey::Digit7, modifiers)),
        56 => Some(plain_mapping(CanonicalKey::Digit8, modifiers)),
        57 => Some(plain_mapping(CanonicalKey::Digit9, modifiers)),
        32 => Some(plain_mapping(CanonicalKey::Space, modifiers)),
        13 | 10 => Some(plain_mapping(CanonicalKey::Enter, modifiers)),
        9 => Some(plain_mapping(CanonicalKey::Tab, modifiers)),
        27 => Some(plain_mapping(CanonicalKey::Escape, modifiers)),
        8 => Some(plain_mapping(CanonicalKey::Backspace, modifiers)),
        127 => Some(plain_mapping(CanonicalKey::Delete, modifiers)),
        45 => Some(plain_mapping(CanonicalKey::Minus, modifiers)),
        61 => Some(plain_mapping(CanonicalKey::Equal, modifiers)),
        91 => Some(plain_mapping(CanonicalKey::BracketLeft, modifiers)),
        93 => Some(plain_mapping(CanonicalKey::BracketRight, modifiers)),
        92 => Some(plain_mapping(CanonicalKey::Backslash, modifiers)),
        59 => Some(plain_mapping(CanonicalKey::Semicolon, modifiers)),
        39 => Some(plain_mapping(CanonicalKey::Quote, modifiers)),
        96 => Some(plain_mapping(CanonicalKey::Backquote, modifiers)),
        44 => Some(plain_mapping(CanonicalKey::Comma, modifiers)),
        46 => Some(plain_mapping(CanonicalKey::Period, modifiers)),
        47 => Some(plain_mapping(CanonicalKey::Slash, modifiers)),
        _ => shifted_printable_char_to_hid(ch, modifiers),
    }
}

fn control_key_to_hid(key: i32) -> Option<u8> {
    match key {
        x if x == ControlKey::Alt as i32 => Some(0xE2), // Left Alt
        x if x == ControlKey::Backspace as i32 => Some(0x2A),
        x if x == ControlKey::CapsLock as i32 => Some(0x39),
        x if x == ControlKey::Control as i32 => Some(0xE0), // Left Ctrl
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
        x if x == ControlKey::Meta as i32 => Some(0xE3), // Left GUI/Windows
        x if x == ControlKey::PageDown as i32 => Some(0x4E),
        x if x == ControlKey::PageUp as i32 => Some(0x4B),
        x if x == ControlKey::Return as i32 => Some(0x28),
        x if x == ControlKey::RightArrow as i32 => Some(0x4F),
        x if x == ControlKey::Shift as i32 => Some(0xE1), // Left Shift
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

fn keycode_to_hid(keycode: u32) -> Option<u8> {
    if let Some(hid) = ascii_to_hid(keycode) {
        return Some(hid);
    }
    if let Some(hid) = windows_vk_to_hid(keycode) {
        return Some(hid);
    }
    x11_keycode_to_hid(keycode)
}

fn ascii_to_hid(ascii: u32) -> Option<u8> {
    match ascii {
        97..=122 => Some((ascii - 97 + 0x04) as u8),
        65..=90 => Some((ascii - 65 + 0x04) as u8),
        48 => Some(0x27),                           // 0
        49..=57 => Some((ascii - 49 + 0x1E) as u8), // 1-9
        32 => Some(0x2C),                           // Space
        13 => Some(0x28),                           // Enter (CR)
        10 => Some(0x28),                           // Enter (LF)
        9 => Some(0x2B),                            // Tab
        27 => Some(0x29),                           // Escape
        8 => Some(0x2A),                            // Backspace
        127 => Some(0x4C),                          // Delete
        45 => Some(0x2D),                           // -
        61 => Some(0x2E),                           // =
        91 => Some(0x2F),                           // [
        93 => Some(0x30),                           // ]
        92 => Some(0x31),                           // \
        59 => Some(0x33),                           // ;
        39 => Some(0x34),                           // '
        96 => Some(0x35),                           // `
        44 => Some(0x36),                           // ,
        46 => Some(0x37),                           // .
        47 => Some(0x38),                           // /
        _ => None,
    }
}

fn windows_vk_to_hid(vk: u32) -> Option<u8> {
    match vk {
        0x41..=0x5A => {
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
        0x30 => Some(0x27),                            // 0
        0x31..=0x39 => Some((vk - 0x31 + 0x1E) as u8), // 1-9
        0x60 => Some(0x62),                            // Numpad 0
        0x61..=0x69 => Some((vk - 0x61 + 0x59) as u8), // Numpad 1-9
        0x6A => Some(0x55),                            // Numpad *
        0x6B => Some(0x57),                            // Numpad +
        0x6D => Some(0x56),                            // Numpad -
        0x6E => Some(0x63),                            // Numpad .
        0x6F => Some(0x54),                            // Numpad /
        0x70..=0x7B => Some((vk - 0x70 + 0x3A) as u8),
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
        0x14 => Some(0x39), // Caps Lock
        0x90 => Some(0x53), // Num Lock
        0x91 => Some(0x47), // Scroll Lock
        0x2C => Some(0x46), // Print Screen
        0x13 => Some(0x48), // Pause
        _ => None,
    }
}

fn x11_keycode_to_hid(keycode: u32) -> Option<u8> {
    match keycode {
        10..=18 => Some((keycode - 10 + 0x1E) as u8), // 1-9
        19 => Some(0x27),                             // 0
        20 => Some(0x2D),                             // -
        21 => Some(0x2E),                             // =
        34 => Some(0x2F),                             // [
        35 => Some(0x30),                             // ]
        24 => Some(0x14),                             // q
        25 => Some(0x1A),                             // w
        26 => Some(0x08),                             // e
        27 => Some(0x15),                             // r
        28 => Some(0x17),                             // t
        29 => Some(0x1C),                             // y
        30 => Some(0x18),                             // u
        31 => Some(0x0C),                             // i
        32 => Some(0x12),                             // o
        33 => Some(0x13),                             // p
        38 => Some(0x04),                             // a
        39 => Some(0x16),                             // s
        40 => Some(0x07),                             // d
        41 => Some(0x09),                             // f
        42 => Some(0x0A),                             // g
        43 => Some(0x0B),                             // h
        44 => Some(0x0D),                             // j
        45 => Some(0x0E),                             // k
        46 => Some(0x0F),                             // l
        47 => Some(0x33),                             // ;
        48 => Some(0x34),                             // '
        49 => Some(0x35),                             // `
        51 => Some(0x31),                             // \
        52 => Some(0x1D),                             // z
        53 => Some(0x1B),                             // x
        54 => Some(0x06),                             // c
        55 => Some(0x19),                             // v
        56 => Some(0x05),                             // b
        57 => Some(0x11),                             // n
        58 => Some(0x10),                             // m
        59 => Some(0x36),                             // ,
        60 => Some(0x37),                             // .
        61 => Some(0x38),                             // /
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
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, MouseEventType::Down);
        assert_eq!(events[0].button, Some(MouseButton::Left));
    }

    #[test]
    fn test_convert_mouse_button_down_does_not_move() {
        let mut event = MouseEvent::new();
        event.mask = (mouse_button::LEFT << 3) | mouse_type::DOWN;

        let events = convert_mouse_event(&event, 1920, 1080);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, MouseEventType::Down);
        assert_eq!(events[0].button, Some(MouseButton::Left));
    }

    #[test]
    fn test_convert_mouse_wheel_does_not_move() {
        let mut event = MouseEvent::new();
        event.x = 500;
        event.y = 1;
        event.mask = mouse_type::WHEEL;

        let events = convert_mouse_event(&event, 1920, 1080);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, MouseEventType::Scroll);
        assert_eq!(events[0].scroll, 1);
    }

    #[test]
    fn test_convert_mouse_move_relative() {
        let mut event = MouseEvent::new();
        event.x = -12;
        event.y = 8;
        event.mask = mouse_type::MOVE_RELATIVE;

        let events = convert_mouse_event(&event, 1920, 1080);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, MouseEventType::Move);
        assert_eq!(events[0].x, -12);
        assert_eq!(events[0].y, 8);
    }

    #[test]
    fn test_convert_key_event() {
        use protobuf::EnumOrUnknown;
        let mut key_event = KeyEvent::new();
        key_event.down = true;
        key_event.press = false;
        key_event.union = Some(ke_union::Union::ControlKey(EnumOrUnknown::new(
            ControlKey::Return,
        )));

        let result = convert_key_event(&key_event);
        assert!(result.is_some());

        let kb_event = result.unwrap();
        assert_eq!(kb_event.event_type, KeyEventType::Down);
        assert_eq!(kb_event.key, CanonicalKey::Enter);
    }

    #[test]
    fn test_convert_at_press_to_shift_digit2() {
        let mut key_event = KeyEvent::new();
        key_event.press = true;
        key_event.union = Some(ke_union::Union::Unicode('@' as u32));

        let events = convert_key_events(&key_event);
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event_type, KeyEventType::Down);
        assert_eq!(events[0].key, CanonicalKey::Digit2);
        assert!(events[0].modifiers.left_shift);
        assert_eq!(events[1].event_type, KeyEventType::Up);
        assert_eq!(events[1].key, CanonicalKey::Digit2);
        assert!(!events[1].modifiers.left_shift);
    }

    #[test]
    fn test_convert_shifted_chr_to_shift_digit2() {
        let mut key_event = KeyEvent::new();
        key_event.down = true;
        key_event.union = Some(ke_union::Union::Chr('@' as u32));

        let events = convert_key_events(&key_event);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, KeyEventType::Down);
        assert_eq!(events[0].key, CanonicalKey::Digit2);
        assert!(events[0].modifiers.left_shift);
    }

    #[test]
    fn test_convert_map_mode_chr_as_physical_key() {
        use protobuf::EnumOrUnknown;
        let mut key_event = KeyEvent::new();
        key_event.down = true;
        key_event.mode = EnumOrUnknown::new(KeyboardMode::Map);
        key_event.union = Some(ke_union::Union::Chr(0x41));

        let events = convert_key_events(&key_event);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, KeyEventType::Down);
        assert_eq!(events[0].key, CanonicalKey::KeyA);
        assert!(!events[0].modifiers.left_shift);
    }

    #[test]
    fn test_convert_press_generates_down_and_up() {
        use protobuf::EnumOrUnknown;
        let mut key_event = KeyEvent::new();
        key_event.press = true;
        key_event.union = Some(ke_union::Union::ControlKey(EnumOrUnknown::new(
            ControlKey::Return,
        )));

        let events = convert_key_events(&key_event);
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event_type, KeyEventType::Down);
        assert_eq!(events[1].event_type, KeyEventType::Up);
        assert_eq!(events[0].key, CanonicalKey::Enter);
        assert_eq!(events[1].key, CanonicalKey::Enter);
    }
}
