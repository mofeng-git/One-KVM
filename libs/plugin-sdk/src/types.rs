//! Keyboard/mouse/consumer structs (`KeyboardEvent`, `MouseEvent`, …).

use serde::{Deserialize, Serialize};

use super::keyboard::CanonicalKey;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum KeyEventType {
    Down,
    Up,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyboardModifiers {
    #[serde(default)]
    pub left_ctrl: bool,
    #[serde(default)]
    pub left_shift: bool,
    #[serde(default)]
    pub left_alt: bool,
    #[serde(default)]
    pub left_meta: bool,
    #[serde(default)]
    pub right_ctrl: bool,
    #[serde(default)]
    pub right_shift: bool,
    #[serde(default)]
    pub right_alt: bool,
    #[serde(default)]
    pub right_meta: bool,
}

impl KeyboardModifiers {
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyboardEvent {
    #[serde(rename = "type")]
    pub event_type: KeyEventType,
    pub key: CanonicalKey,
    #[serde(default)]
    pub modifiers: KeyboardModifiers,
}

impl KeyboardEvent {
    pub fn key_down(key: CanonicalKey, modifiers: KeyboardModifiers) -> Self {
        Self {
            event_type: KeyEventType::Down,
            key,
            modifiers,
        }
    }

    pub fn key_up(key: CanonicalKey, modifiers: KeyboardModifiers) -> Self {
        Self {
            event_type: KeyEventType::Up,
            key,
            modifiers,
        }
    }
}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MouseEventType {
    Move,
    MoveAbs,
    Down,
    Up,
    Scroll,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MouseEvent {
    #[serde(rename = "type")]
    pub event_type: MouseEventType,
    #[serde(default)]
    pub x: i32,
    #[serde(default)]
    pub y: i32,
    #[serde(default)]
    pub button: Option<MouseButton>,
    #[serde(default)]
    pub scroll: i8,
}

impl MouseEvent {
    pub fn move_rel(dx: i32, dy: i32) -> Self {
        Self {
            event_type: MouseEventType::Move,
            x: dx,
            y: dy,
            button: None,
            scroll: 0,
        }
    }

    pub fn move_abs(x: i32, y: i32) -> Self {
        Self {
            event_type: MouseEventType::MoveAbs,
            x,
            y,
            button: None,
            scroll: 0,
        }
    }

    pub fn button_down(button: MouseButton) -> Self {
        Self {
            event_type: MouseEventType::Down,
            x: 0,
            y: 0,
            button: Some(button),
            scroll: 0,
        }
    }

    pub fn button_up(button: MouseButton) -> Self {
        Self {
            event_type: MouseEventType::Up,
            x: 0,
            y: 0,
            button: Some(button),
            scroll: 0,
        }
    }

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsumerEvent {
    pub usage: u16,
}

#[derive(Debug, Clone, Default)]
pub struct KeyboardReport {
    pub modifiers: u8,
    pub reserved: u8,
    pub keys: [u8; 6],
}

impl KeyboardReport {
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

    pub fn add_key(&mut self, key: u8) -> bool {
        for slot in &mut self.keys {
            if *slot == 0 {
                *slot = key;
                return true;
            }
        }
        false // All slots full
    }

    pub fn remove_key(&mut self, key: u8) {
        for slot in &mut self.keys {
            if *slot == key {
                *slot = 0;
            }
        }
        self.keys.sort_by(|a, b| b.cmp(a));
    }

    pub fn clear(&mut self) {
        self.modifiers = 0;
        self.keys = [0; 6];
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
