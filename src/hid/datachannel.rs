//! DataChannel HID message parsing and handling
//!
//! Binary message format:
//! - Byte 0: Message type
//!   - 0x01: Keyboard event
//!   - 0x02: Mouse event
//! - Remaining bytes: Event data
//!
//! Keyboard event (type 0x01):
//! - Byte 1: Event type (0x00 = down, 0x01 = up)
//! - Byte 2: Key code (USB HID usage code or JS keyCode)
//! - Byte 3: Modifiers bitmask
//!   - Bit 0: Left Ctrl
//!   - Bit 1: Left Shift
//!   - Bit 2: Left Alt
//!   - Bit 3: Left Meta
//!   - Bit 4: Right Ctrl
//!   - Bit 5: Right Shift
//!   - Bit 6: Right Alt
//!   - Bit 7: Right Meta
//!
//! Mouse event (type 0x02):
//! - Byte 1: Event type
//!   - 0x00: Move (relative)
//!   - 0x01: MoveAbs (absolute)
//!   - 0x02: Down
//!   - 0x03: Up
//!   - 0x04: Scroll
//! - Bytes 2-3: X coordinate (i16 LE for relative, u16 LE for absolute)
//! - Bytes 4-5: Y coordinate (i16 LE for relative, u16 LE for absolute)
//! - Byte 6: Button (0=left, 1=middle, 2=right) or Scroll delta (i8)

use tracing::{debug, warn};

use super::{
    KeyEventType, KeyboardEvent, KeyboardModifiers, MouseButton, MouseEvent, MouseEventType,
};

/// Message types
pub const MSG_KEYBOARD: u8 = 0x01;
pub const MSG_MOUSE: u8 = 0x02;

/// Keyboard event types
pub const KB_EVENT_DOWN: u8 = 0x00;
pub const KB_EVENT_UP: u8 = 0x01;

/// Mouse event types
pub const MS_EVENT_MOVE: u8 = 0x00;
pub const MS_EVENT_MOVE_ABS: u8 = 0x01;
pub const MS_EVENT_DOWN: u8 = 0x02;
pub const MS_EVENT_UP: u8 = 0x03;
pub const MS_EVENT_SCROLL: u8 = 0x04;

/// Parsed HID event from DataChannel
#[derive(Debug, Clone)]
pub enum HidChannelEvent {
    Keyboard(KeyboardEvent),
    Mouse(MouseEvent),
}

/// Parse a binary HID message from DataChannel
pub fn parse_hid_message(data: &[u8]) -> Option<HidChannelEvent> {
    if data.is_empty() {
        warn!("Empty HID message");
        return None;
    }

    let msg_type = data[0];

    match msg_type {
        MSG_KEYBOARD => parse_keyboard_message(&data[1..]),
        MSG_MOUSE => parse_mouse_message(&data[1..]),
        _ => {
            warn!("Unknown HID message type: 0x{:02X}", msg_type);
            None
        }
    }
}

/// Parse keyboard message payload
fn parse_keyboard_message(data: &[u8]) -> Option<HidChannelEvent> {
    if data.len() < 3 {
        warn!("Keyboard message too short: {} bytes", data.len());
        return None;
    }

    let event_type = match data[0] {
        KB_EVENT_DOWN => KeyEventType::Down,
        KB_EVENT_UP => KeyEventType::Up,
        _ => {
            warn!("Unknown keyboard event type: 0x{:02X}", data[0]);
            return None;
        }
    };

    let key = data[1];
    let modifiers_byte = data[2];

    let modifiers = KeyboardModifiers {
        left_ctrl: modifiers_byte & 0x01 != 0,
        left_shift: modifiers_byte & 0x02 != 0,
        left_alt: modifiers_byte & 0x04 != 0,
        left_meta: modifiers_byte & 0x08 != 0,
        right_ctrl: modifiers_byte & 0x10 != 0,
        right_shift: modifiers_byte & 0x20 != 0,
        right_alt: modifiers_byte & 0x40 != 0,
        right_meta: modifiers_byte & 0x80 != 0,
    };

    debug!(
        "Parsed keyboard: {:?} key=0x{:02X} modifiers=0x{:02X}",
        event_type, key, modifiers_byte
    );

    Some(HidChannelEvent::Keyboard(KeyboardEvent {
        event_type,
        key,
        modifiers,
    }))
}

/// Parse mouse message payload
fn parse_mouse_message(data: &[u8]) -> Option<HidChannelEvent> {
    if data.len() < 6 {
        warn!("Mouse message too short: {} bytes", data.len());
        return None;
    }

    let event_type = match data[0] {
        MS_EVENT_MOVE => MouseEventType::Move,
        MS_EVENT_MOVE_ABS => MouseEventType::MoveAbs,
        MS_EVENT_DOWN => MouseEventType::Down,
        MS_EVENT_UP => MouseEventType::Up,
        MS_EVENT_SCROLL => MouseEventType::Scroll,
        _ => {
            warn!("Unknown mouse event type: 0x{:02X}", data[0]);
            return None;
        }
    };

    // Parse coordinates as i16 LE (works for both relative and absolute)
    let x = i16::from_le_bytes([data[1], data[2]]) as i32;
    let y = i16::from_le_bytes([data[3], data[4]]) as i32;

    // Button or scroll delta
    let (button, scroll) = match event_type {
        MouseEventType::Down | MouseEventType::Up => {
            let btn = match data[5] {
                0 => Some(MouseButton::Left),
                1 => Some(MouseButton::Middle),
                2 => Some(MouseButton::Right),
                3 => Some(MouseButton::Back),
                4 => Some(MouseButton::Forward),
                _ => Some(MouseButton::Left),
            };
            (btn, 0i8)
        }
        MouseEventType::Scroll => (None, data[5] as i8),
        _ => (None, 0i8),
    };

    debug!(
        "Parsed mouse: {:?} x={} y={} button={:?} scroll={}",
        event_type, x, y, button, scroll
    );

    Some(HidChannelEvent::Mouse(MouseEvent {
        event_type,
        x,
        y,
        button,
        scroll,
    }))
}

/// Encode a keyboard event to binary format (for sending to client if needed)
pub fn encode_keyboard_event(event: &KeyboardEvent) -> Vec<u8> {
    let event_type = match event.event_type {
        KeyEventType::Down => KB_EVENT_DOWN,
        KeyEventType::Up => KB_EVENT_UP,
    };

    let modifiers = event.modifiers.to_hid_byte();

    vec![MSG_KEYBOARD, event_type, event.key, modifiers]
}

/// Encode a mouse event to binary format (for sending to client if needed)
pub fn encode_mouse_event(event: &MouseEvent) -> Vec<u8> {
    let event_type = match event.event_type {
        MouseEventType::Move => MS_EVENT_MOVE,
        MouseEventType::MoveAbs => MS_EVENT_MOVE_ABS,
        MouseEventType::Down => MS_EVENT_DOWN,
        MouseEventType::Up => MS_EVENT_UP,
        MouseEventType::Scroll => MS_EVENT_SCROLL,
    };

    let x_bytes = (event.x as i16).to_le_bytes();
    let y_bytes = (event.y as i16).to_le_bytes();

    let extra = match event.event_type {
        MouseEventType::Down | MouseEventType::Up => {
            event.button.as_ref().map(|b| match b {
                MouseButton::Left => 0u8,
                MouseButton::Middle => 1u8,
                MouseButton::Right => 2u8,
                MouseButton::Back => 3u8,
                MouseButton::Forward => 4u8,
            }).unwrap_or(0)
        }
        MouseEventType::Scroll => event.scroll as u8,
        _ => 0,
    };

    vec![
        MSG_MOUSE,
        event_type,
        x_bytes[0],
        x_bytes[1],
        y_bytes[0],
        y_bytes[1],
        extra,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_keyboard_down() {
        let data = [MSG_KEYBOARD, KB_EVENT_DOWN, 0x04, 0x01]; // A key with left ctrl
        let event = parse_hid_message(&data).unwrap();

        match event {
            HidChannelEvent::Keyboard(kb) => {
                assert!(matches!(kb.event_type, KeyEventType::Down));
                assert_eq!(kb.key, 0x04);
                assert!(kb.modifiers.left_ctrl);
                assert!(!kb.modifiers.left_shift);
            }
            _ => panic!("Expected keyboard event"),
        }
    }

    #[test]
    fn test_parse_mouse_move() {
        let data = [MSG_MOUSE, MS_EVENT_MOVE, 0x0A, 0x00, 0xF6, 0xFF, 0x00]; // x=10, y=-10
        let event = parse_hid_message(&data).unwrap();

        match event {
            HidChannelEvent::Mouse(ms) => {
                assert!(matches!(ms.event_type, MouseEventType::Move));
                assert_eq!(ms.x, 10);
                assert_eq!(ms.y, -10);
            }
            _ => panic!("Expected mouse event"),
        }
    }

    #[test]
    fn test_encode_keyboard() {
        let event = KeyboardEvent {
            event_type: KeyEventType::Down,
            key: 0x04,
            modifiers: KeyboardModifiers {
                left_ctrl: true,
                left_shift: false,
                left_alt: false,
                left_meta: false,
                right_ctrl: false,
                right_shift: false,
                right_alt: false,
                right_meta: false,
            },
        };

        let encoded = encode_keyboard_event(&event);
        assert_eq!(encoded, vec![MSG_KEYBOARD, KB_EVENT_DOWN, 0x04, 0x01]);
    }
}
