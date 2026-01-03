//! HID event types for keyboard and mouse

use serde::{Deserialize, Serialize};

/// Keyboard event type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum KeyEventType {
    /// Key pressed down
    Down,
    /// Key released
    Up,
}

/// Keyboard modifier flags
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyboardModifiers {
    /// Left Control
    #[serde(default)]
    pub left_ctrl: bool,
    /// Left Shift
    #[serde(default)]
    pub left_shift: bool,
    /// Left Alt
    #[serde(default)]
    pub left_alt: bool,
    /// Left Meta (Windows/Super key)
    #[serde(default)]
    pub left_meta: bool,
    /// Right Control
    #[serde(default)]
    pub right_ctrl: bool,
    /// Right Shift
    #[serde(default)]
    pub right_shift: bool,
    /// Right Alt (AltGr)
    #[serde(default)]
    pub right_alt: bool,
    /// Right Meta
    #[serde(default)]
    pub right_meta: bool,
}

impl KeyboardModifiers {
    /// Convert to USB HID modifier byte
    pub fn to_hid_byte(&self) -> u8 {
        let mut byte = 0u8;
        if self.left_ctrl {
            byte |= 0x01;
        }
        if self.left_shift {
            byte |= 0x02;
        }
        if self.left_alt {
            byte |= 0x04;
        }
        if self.left_meta {
            byte |= 0x08;
        }
        if self.right_ctrl {
            byte |= 0x10;
        }
        if self.right_shift {
            byte |= 0x20;
        }
        if self.right_alt {
            byte |= 0x40;
        }
        if self.right_meta {
            byte |= 0x80;
        }
        byte
    }

    /// Create from USB HID modifier byte
    pub fn from_hid_byte(byte: u8) -> Self {
        Self {
            left_ctrl: byte & 0x01 != 0,
            left_shift: byte & 0x02 != 0,
            left_alt: byte & 0x04 != 0,
            left_meta: byte & 0x08 != 0,
            right_ctrl: byte & 0x10 != 0,
            right_shift: byte & 0x20 != 0,
            right_alt: byte & 0x40 != 0,
            right_meta: byte & 0x80 != 0,
        }
    }

    /// Check if any modifier is active
    pub fn any(&self) -> bool {
        self.left_ctrl
            || self.left_shift
            || self.left_alt
            || self.left_meta
            || self.right_ctrl
            || self.right_shift
            || self.right_alt
            || self.right_meta
    }
}

/// Keyboard event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyboardEvent {
    /// Event type (down/up)
    #[serde(rename = "type")]
    pub event_type: KeyEventType,
    /// Key code (USB HID usage code or JavaScript key code)
    pub key: u8,
    /// Modifier keys state
    #[serde(default)]
    pub modifiers: KeyboardModifiers,
    /// If true, key is already USB HID code (skip js_to_usb conversion)
    #[serde(default)]
    pub is_usb_hid: bool,
}

impl KeyboardEvent {
    /// Create a key down event (JS keycode, needs conversion)
    pub fn key_down(key: u8, modifiers: KeyboardModifiers) -> Self {
        Self {
            event_type: KeyEventType::Down,
            key,
            modifiers,
            is_usb_hid: false,
        }
    }

    /// Create a key up event (JS keycode, needs conversion)
    pub fn key_up(key: u8, modifiers: KeyboardModifiers) -> Self {
        Self {
            event_type: KeyEventType::Up,
            key,
            modifiers,
            is_usb_hid: false,
        }
    }
}

/// Mouse button
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    Back,
    Forward,
}

impl MouseButton {
    /// Convert to USB HID button bit
    pub fn to_hid_bit(&self) -> u8 {
        match self {
            MouseButton::Left => 0x01,
            MouseButton::Right => 0x02,
            MouseButton::Middle => 0x04,
            MouseButton::Back => 0x08,
            MouseButton::Forward => 0x10,
        }
    }
}

/// Mouse event type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MouseEventType {
    /// Mouse moved (relative movement)
    Move,
    /// Mouse moved (absolute position)
    MoveAbs,
    /// Button pressed
    Down,
    /// Button released
    Up,
    /// Mouse wheel scroll
    Scroll,
}

/// Mouse event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MouseEvent {
    /// Event type
    #[serde(rename = "type")]
    pub event_type: MouseEventType,
    /// X coordinate or delta
    #[serde(default)]
    pub x: i32,
    /// Y coordinate or delta
    #[serde(default)]
    pub y: i32,
    /// Button (for down/up events)
    #[serde(default)]
    pub button: Option<MouseButton>,
    /// Scroll delta (for scroll events)
    #[serde(default)]
    pub scroll: i8,
}

impl MouseEvent {
    /// Create a relative move event
    pub fn move_rel(dx: i32, dy: i32) -> Self {
        Self {
            event_type: MouseEventType::Move,
            x: dx,
            y: dy,
            button: None,
            scroll: 0,
        }
    }

    /// Create an absolute move event
    pub fn move_abs(x: i32, y: i32) -> Self {
        Self {
            event_type: MouseEventType::MoveAbs,
            x,
            y,
            button: None,
            scroll: 0,
        }
    }

    /// Create a button down event
    pub fn button_down(button: MouseButton) -> Self {
        Self {
            event_type: MouseEventType::Down,
            x: 0,
            y: 0,
            button: Some(button),
            scroll: 0,
        }
    }

    /// Create a button up event
    pub fn button_up(button: MouseButton) -> Self {
        Self {
            event_type: MouseEventType::Up,
            x: 0,
            y: 0,
            button: Some(button),
            scroll: 0,
        }
    }

    /// Create a scroll event
    pub fn scroll(delta: i8) -> Self {
        Self {
            event_type: MouseEventType::Scroll,
            x: 0,
            y: 0,
            button: None,
            scroll: delta,
        }
    }
}

/// Combined HID event (keyboard or mouse)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "device", rename_all = "lowercase")]
pub enum HidEvent {
    Keyboard(KeyboardEvent),
    Mouse(MouseEvent),
    Consumer(ConsumerEvent),
}

/// Consumer control event (multimedia keys)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsumerEvent {
    /// Consumer control usage code (e.g., 0x00CD for Play/Pause)
    pub usage: u16,
}

/// USB HID keyboard report (8 bytes)
#[derive(Debug, Clone, Default)]
pub struct KeyboardReport {
    /// Modifier byte
    pub modifiers: u8,
    /// Reserved byte
    pub reserved: u8,
    /// Key codes (up to 6 simultaneous keys)
    pub keys: [u8; 6],
}

impl KeyboardReport {
    /// Convert to bytes for USB HID
    pub fn to_bytes(&self) -> [u8; 8] {
        [
            self.modifiers,
            self.reserved,
            self.keys[0],
            self.keys[1],
            self.keys[2],
            self.keys[3],
            self.keys[4],
            self.keys[5],
        ]
    }

    /// Add a key to the report
    pub fn add_key(&mut self, key: u8) -> bool {
        for slot in &mut self.keys {
            if *slot == 0 {
                *slot = key;
                return true;
            }
        }
        false // All slots full
    }

    /// Remove a key from the report
    pub fn remove_key(&mut self, key: u8) {
        for slot in &mut self.keys {
            if *slot == key {
                *slot = 0;
            }
        }
        // Compact the array
        self.keys.sort_by(|a, b| b.cmp(a));
    }

    /// Clear all keys
    pub fn clear(&mut self) {
        self.modifiers = 0;
        self.keys = [0; 6];
    }
}

/// USB HID mouse report
#[derive(Debug, Clone, Default)]
pub struct MouseReport {
    /// Button state
    pub buttons: u8,
    /// X movement (-127 to 127)
    pub x: i8,
    /// Y movement (-127 to 127)
    pub y: i8,
    /// Wheel movement (-127 to 127)
    pub wheel: i8,
}

impl MouseReport {
    /// Convert to bytes for USB HID (relative mouse)
    pub fn to_bytes_relative(&self) -> [u8; 4] {
        [
            self.buttons,
            self.x as u8,
            self.y as u8,
            self.wheel as u8,
        ]
    }

    /// Convert to bytes for USB HID (absolute mouse)
    pub fn to_bytes_absolute(&self, x: u16, y: u16) -> [u8; 6] {
        [
            self.buttons,
            (x & 0xFF) as u8,
            (x >> 8) as u8,
            (y & 0xFF) as u8,
            (y >> 8) as u8,
            self.wheel as u8,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_modifier_conversion() {
        let mods = KeyboardModifiers {
            left_ctrl: true,
            left_shift: true,
            ..Default::default()
        };
        assert_eq!(mods.to_hid_byte(), 0x03);

        let mods2 = KeyboardModifiers::from_hid_byte(0x03);
        assert!(mods2.left_ctrl);
        assert!(mods2.left_shift);
        assert!(!mods2.left_alt);
    }

    #[test]
    fn test_keyboard_report() {
        let mut report = KeyboardReport::default();
        assert!(report.add_key(0x04)); // 'A'
        assert!(report.add_key(0x05)); // 'B'
        assert_eq!(report.keys[0], 0x04);
        assert_eq!(report.keys[1], 0x05);

        report.remove_key(0x04);
        assert_eq!(report.keys[0], 0x05);
    }
}
