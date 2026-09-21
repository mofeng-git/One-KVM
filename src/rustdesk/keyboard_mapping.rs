//! RustDesk keyboard code-space mappings.
//!
//! Keep physical position codes separate from character and control-key values. In
//! particular, a Windows Set-1 scan code must never fall through to ASCII, VK, or
//! X11 interpretation.

use super::protocol::ControlKey;
use crate::hid::CanonicalKey;

/// Physical keyboard code space selected by the platform advertised to RustDesk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum KeyboardCodeSpace {
    WindowsSet1,
}

pub(super) fn physical_key(code_space: KeyboardCodeSpace, code: u32) -> Option<CanonicalKey> {
    match code_space {
        KeyboardCodeSpace::WindowsSet1 => windows_set1_key(code),
    }
}

/// Convert a Windows Set-1 make code as encoded by RustDesk.
///
/// Ordinary codes occupy the low byte. Extended codes are encoded as `0xE0xx`.
/// Other multi-byte values (including the special Pause sequence) are rejected.
pub(super) fn windows_set1_key(scan_code: u32) -> Option<CanonicalKey> {
    use CanonicalKey as Key;

    match scan_code {
        0x01 => Some(Key::Escape),
        0x02 => Some(Key::Digit1),
        0x03 => Some(Key::Digit2),
        0x04 => Some(Key::Digit3),
        0x05 => Some(Key::Digit4),
        0x06 => Some(Key::Digit5),
        0x07 => Some(Key::Digit6),
        0x08 => Some(Key::Digit7),
        0x09 => Some(Key::Digit8),
        0x0A => Some(Key::Digit9),
        0x0B => Some(Key::Digit0),
        0x0C => Some(Key::Minus),
        0x0D => Some(Key::Equal),
        0x0E => Some(Key::Backspace),
        0x0F => Some(Key::Tab),
        0x10 => Some(Key::KeyQ),
        0x11 => Some(Key::KeyW),
        0x12 => Some(Key::KeyE),
        0x13 => Some(Key::KeyR),
        0x14 => Some(Key::KeyT),
        0x15 => Some(Key::KeyY),
        0x16 => Some(Key::KeyU),
        0x17 => Some(Key::KeyI),
        0x18 => Some(Key::KeyO),
        0x19 => Some(Key::KeyP),
        0x1A => Some(Key::BracketLeft),
        0x1B => Some(Key::BracketRight),
        0x1C => Some(Key::Enter),
        0x1D => Some(Key::ControlLeft),
        0x1E => Some(Key::KeyA),
        0x1F => Some(Key::KeyS),
        0x20 => Some(Key::KeyD),
        0x21 => Some(Key::KeyF),
        0x22 => Some(Key::KeyG),
        0x23 => Some(Key::KeyH),
        0x24 => Some(Key::KeyJ),
        0x25 => Some(Key::KeyK),
        0x26 => Some(Key::KeyL),
        0x27 => Some(Key::Semicolon),
        0x28 => Some(Key::Quote),
        0x29 => Some(Key::Backquote),
        0x2A => Some(Key::ShiftLeft),
        0x2B => Some(Key::Backslash),
        0x2C => Some(Key::KeyZ),
        0x2D => Some(Key::KeyX),
        0x2E => Some(Key::KeyC),
        0x2F => Some(Key::KeyV),
        0x30 => Some(Key::KeyB),
        0x31 => Some(Key::KeyN),
        0x32 => Some(Key::KeyM),
        0x33 => Some(Key::Comma),
        0x34 => Some(Key::Period),
        0x35 => Some(Key::Slash),
        0x36 => Some(Key::ShiftRight),
        0x37 => Some(Key::NumpadMultiply),
        0x38 => Some(Key::AltLeft),
        0x39 => Some(Key::Space),
        0x3A => Some(Key::CapsLock),
        0x3B => Some(Key::F1),
        0x3C => Some(Key::F2),
        0x3D => Some(Key::F3),
        0x3E => Some(Key::F4),
        0x3F => Some(Key::F5),
        0x40 => Some(Key::F6),
        0x41 => Some(Key::F7),
        0x42 => Some(Key::F8),
        0x43 => Some(Key::F9),
        0x44 => Some(Key::F10),
        0x45 => Some(Key::NumLock),
        0x46 => Some(Key::ScrollLock),
        0x47 => Some(Key::Numpad7),
        0x48 => Some(Key::Numpad8),
        0x49 => Some(Key::Numpad9),
        0x4A => Some(Key::NumpadSubtract),
        0x4B => Some(Key::Numpad4),
        0x4C => Some(Key::Numpad5),
        0x4D => Some(Key::Numpad6),
        0x4E => Some(Key::NumpadAdd),
        0x4F => Some(Key::Numpad1),
        0x50 => Some(Key::Numpad2),
        0x51 => Some(Key::Numpad3),
        0x52 => Some(Key::Numpad0),
        0x53 => Some(Key::NumpadDecimal),
        0x56 => Some(Key::IntlBackslash),
        0x57 => Some(Key::F11),
        0x58 => Some(Key::F12),

        0xE01C => Some(Key::NumpadEnter),
        0xE01D => Some(Key::ControlRight),
        0xE035 => Some(Key::NumpadDivide),
        0xE037 => Some(Key::PrintScreen),
        0xE038 => Some(Key::AltRight),
        0xE047 => Some(Key::Home),
        0xE048 => Some(Key::ArrowUp),
        0xE049 => Some(Key::PageUp),
        0xE04B => Some(Key::ArrowLeft),
        0xE04D => Some(Key::ArrowRight),
        0xE04F => Some(Key::End),
        0xE050 => Some(Key::ArrowDown),
        0xE051 => Some(Key::PageDown),
        0xE052 => Some(Key::Insert),
        0xE053 => Some(Key::Delete),
        0xE05B => Some(Key::MetaLeft),
        0xE05C => Some(Key::MetaRight),
        0xE05D => Some(Key::ContextMenu),
        _ => None,
    }
}

pub(super) fn control_key(key: i32) -> Option<CanonicalKey> {
    use CanonicalKey as Key;

    match key {
        x if x == ControlKey::Alt as i32 => Some(Key::AltLeft),
        x if x == ControlKey::Backspace as i32 => Some(Key::Backspace),
        x if x == ControlKey::CapsLock as i32 => Some(Key::CapsLock),
        x if x == ControlKey::Control as i32 => Some(Key::ControlLeft),
        x if x == ControlKey::Delete as i32 => Some(Key::Delete),
        x if x == ControlKey::DownArrow as i32 => Some(Key::ArrowDown),
        x if x == ControlKey::End as i32 => Some(Key::End),
        x if x == ControlKey::Escape as i32 => Some(Key::Escape),
        x if x == ControlKey::F1 as i32 => Some(Key::F1),
        x if x == ControlKey::F2 as i32 => Some(Key::F2),
        x if x == ControlKey::F3 as i32 => Some(Key::F3),
        x if x == ControlKey::F4 as i32 => Some(Key::F4),
        x if x == ControlKey::F5 as i32 => Some(Key::F5),
        x if x == ControlKey::F6 as i32 => Some(Key::F6),
        x if x == ControlKey::F7 as i32 => Some(Key::F7),
        x if x == ControlKey::F8 as i32 => Some(Key::F8),
        x if x == ControlKey::F9 as i32 => Some(Key::F9),
        x if x == ControlKey::F10 as i32 => Some(Key::F10),
        x if x == ControlKey::F11 as i32 => Some(Key::F11),
        x if x == ControlKey::F12 as i32 => Some(Key::F12),
        x if x == ControlKey::Home as i32 => Some(Key::Home),
        x if x == ControlKey::LeftArrow as i32 => Some(Key::ArrowLeft),
        x if x == ControlKey::Meta as i32 => Some(Key::MetaLeft),
        x if x == ControlKey::PageDown as i32 => Some(Key::PageDown),
        x if x == ControlKey::PageUp as i32 => Some(Key::PageUp),
        x if x == ControlKey::Return as i32 => Some(Key::Enter),
        x if x == ControlKey::RightArrow as i32 => Some(Key::ArrowRight),
        x if x == ControlKey::Shift as i32 => Some(Key::ShiftLeft),
        x if x == ControlKey::Space as i32 => Some(Key::Space),
        x if x == ControlKey::Tab as i32 => Some(Key::Tab),
        x if x == ControlKey::UpArrow as i32 => Some(Key::ArrowUp),
        x if x == ControlKey::Numpad0 as i32 => Some(Key::Numpad0),
        x if x == ControlKey::Numpad1 as i32 => Some(Key::Numpad1),
        x if x == ControlKey::Numpad2 as i32 => Some(Key::Numpad2),
        x if x == ControlKey::Numpad3 as i32 => Some(Key::Numpad3),
        x if x == ControlKey::Numpad4 as i32 => Some(Key::Numpad4),
        x if x == ControlKey::Numpad5 as i32 => Some(Key::Numpad5),
        x if x == ControlKey::Numpad6 as i32 => Some(Key::Numpad6),
        x if x == ControlKey::Numpad7 as i32 => Some(Key::Numpad7),
        x if x == ControlKey::Numpad8 as i32 => Some(Key::Numpad8),
        x if x == ControlKey::Numpad9 as i32 => Some(Key::Numpad9),
        x if x == ControlKey::Pause as i32 => Some(Key::Pause),
        x if x == ControlKey::Snapshot as i32 => Some(Key::PrintScreen),
        x if x == ControlKey::Insert as i32 => Some(Key::Insert),
        x if x == ControlKey::Scroll as i32 => Some(Key::ScrollLock),
        x if x == ControlKey::NumLock as i32 => Some(Key::NumLock),
        x if x == ControlKey::RWin as i32 => Some(Key::MetaRight),
        x if x == ControlKey::Apps as i32 => Some(Key::ContextMenu),
        x if x == ControlKey::Multiply as i32 => Some(Key::NumpadMultiply),
        x if x == ControlKey::Add as i32 => Some(Key::NumpadAdd),
        x if x == ControlKey::Subtract as i32 => Some(Key::NumpadSubtract),
        x if x == ControlKey::Decimal as i32 => Some(Key::NumpadDecimal),
        x if x == ControlKey::Divide as i32 => Some(Key::NumpadDivide),
        x if x == ControlKey::NumpadEnter as i32 => Some(Key::NumpadEnter),
        x if x == ControlKey::RShift as i32 => Some(Key::ShiftRight),
        x if x == ControlKey::RControl as i32 => Some(Key::ControlRight),
        x if x == ControlKey::RAlt as i32 => Some(Key::AltRight),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct CharacterMapping {
    pub key: CanonicalKey,
    pub needs_shift: bool,
}

pub(super) fn character(ch: u32) -> Option<CharacterMapping> {
    use CanonicalKey as Key;

    let plain = |key| CharacterMapping {
        key,
        needs_shift: false,
    };
    let shifted = |key| CharacterMapping {
        key,
        needs_shift: true,
    };

    Some(match ch {
        0x61..=0x7A => plain(CanonicalKey::from_hid_usage((ch - 0x61 + 0x04) as u8)?),
        0x41..=0x5A => shifted(CanonicalKey::from_hid_usage((ch - 0x41 + 0x04) as u8)?),
        0x30 => plain(Key::Digit0),
        0x31 => plain(Key::Digit1),
        0x32 => plain(Key::Digit2),
        0x33 => plain(Key::Digit3),
        0x34 => plain(Key::Digit4),
        0x35 => plain(Key::Digit5),
        0x36 => plain(Key::Digit6),
        0x37 => plain(Key::Digit7),
        0x38 => plain(Key::Digit8),
        0x39 => plain(Key::Digit9),
        0x20 => plain(Key::Space),
        0x0D | 0x0A => plain(Key::Enter),
        0x09 => plain(Key::Tab),
        0x1B => plain(Key::Escape),
        0x08 => plain(Key::Backspace),
        0x7F => plain(Key::Delete),
        0x2D => plain(Key::Minus),
        0x3D => plain(Key::Equal),
        0x5B => plain(Key::BracketLeft),
        0x5D => plain(Key::BracketRight),
        0x5C => plain(Key::Backslash),
        0x3B => plain(Key::Semicolon),
        0x27 => plain(Key::Quote),
        0x60 => plain(Key::Backquote),
        0x2C => plain(Key::Comma),
        0x2E => plain(Key::Period),
        0x2F => plain(Key::Slash),
        0x21 => shifted(Key::Digit1),
        0x40 => shifted(Key::Digit2),
        0x23 => shifted(Key::Digit3),
        0x24 => shifted(Key::Digit4),
        0x25 => shifted(Key::Digit5),
        0x5E => shifted(Key::Digit6),
        0x26 => shifted(Key::Digit7),
        0x2A => shifted(Key::Digit8),
        0x28 => shifted(Key::Digit9),
        0x29 => shifted(Key::Digit0),
        0x5F => shifted(Key::Minus),
        0x2B => shifted(Key::Equal),
        0x7B => shifted(Key::BracketLeft),
        0x7D => shifted(Key::BracketRight),
        0x7C => shifted(Key::Backslash),
        0x3A => shifted(Key::Semicolon),
        0x22 => shifted(Key::Quote),
        0x7E => shifted(Key::Backquote),
        0x3C => shifted(Key::Comma),
        0x3E => shifted(Key::Period),
        0x3F => shifted(Key::Slash),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_set1_matrix_is_complete() {
        use CanonicalKey as Key;

        let cases = [
            (0x01, Key::Escape),
            (0x02, Key::Digit1),
            (0x03, Key::Digit2),
            (0x04, Key::Digit3),
            (0x05, Key::Digit4),
            (0x06, Key::Digit5),
            (0x07, Key::Digit6),
            (0x08, Key::Digit7),
            (0x09, Key::Digit8),
            (0x0A, Key::Digit9),
            (0x0B, Key::Digit0),
            (0x0C, Key::Minus),
            (0x0D, Key::Equal),
            (0x0E, Key::Backspace),
            (0x0F, Key::Tab),
            (0x10, Key::KeyQ),
            (0x11, Key::KeyW),
            (0x12, Key::KeyE),
            (0x13, Key::KeyR),
            (0x14, Key::KeyT),
            (0x15, Key::KeyY),
            (0x16, Key::KeyU),
            (0x17, Key::KeyI),
            (0x18, Key::KeyO),
            (0x19, Key::KeyP),
            (0x1A, Key::BracketLeft),
            (0x1B, Key::BracketRight),
            (0x1C, Key::Enter),
            (0x1D, Key::ControlLeft),
            (0x1E, Key::KeyA),
            (0x1F, Key::KeyS),
            (0x20, Key::KeyD),
            (0x21, Key::KeyF),
            (0x22, Key::KeyG),
            (0x23, Key::KeyH),
            (0x24, Key::KeyJ),
            (0x25, Key::KeyK),
            (0x26, Key::KeyL),
            (0x27, Key::Semicolon),
            (0x28, Key::Quote),
            (0x29, Key::Backquote),
            (0x2A, Key::ShiftLeft),
            (0x2B, Key::Backslash),
            (0x2C, Key::KeyZ),
            (0x2D, Key::KeyX),
            (0x2E, Key::KeyC),
            (0x2F, Key::KeyV),
            (0x30, Key::KeyB),
            (0x31, Key::KeyN),
            (0x32, Key::KeyM),
            (0x33, Key::Comma),
            (0x34, Key::Period),
            (0x35, Key::Slash),
            (0x36, Key::ShiftRight),
            (0x37, Key::NumpadMultiply),
            (0x38, Key::AltLeft),
            (0x39, Key::Space),
            (0x3A, Key::CapsLock),
            (0x3B, Key::F1),
            (0x3C, Key::F2),
            (0x3D, Key::F3),
            (0x3E, Key::F4),
            (0x3F, Key::F5),
            (0x40, Key::F6),
            (0x41, Key::F7),
            (0x42, Key::F8),
            (0x43, Key::F9),
            (0x44, Key::F10),
            (0x45, Key::NumLock),
            (0x46, Key::ScrollLock),
            (0x47, Key::Numpad7),
            (0x48, Key::Numpad8),
            (0x49, Key::Numpad9),
            (0x4A, Key::NumpadSubtract),
            (0x4B, Key::Numpad4),
            (0x4C, Key::Numpad5),
            (0x4D, Key::Numpad6),
            (0x4E, Key::NumpadAdd),
            (0x4F, Key::Numpad1),
            (0x50, Key::Numpad2),
            (0x51, Key::Numpad3),
            (0x52, Key::Numpad0),
            (0x53, Key::NumpadDecimal),
            (0x56, Key::IntlBackslash),
            (0x57, Key::F11),
            (0x58, Key::F12),
            (0xE01C, Key::NumpadEnter),
            (0xE01D, Key::ControlRight),
            (0xE035, Key::NumpadDivide),
            (0xE037, Key::PrintScreen),
            (0xE038, Key::AltRight),
            (0xE047, Key::Home),
            (0xE048, Key::ArrowUp),
            (0xE049, Key::PageUp),
            (0xE04B, Key::ArrowLeft),
            (0xE04D, Key::ArrowRight),
            (0xE04F, Key::End),
            (0xE050, Key::ArrowDown),
            (0xE051, Key::PageDown),
            (0xE052, Key::Insert),
            (0xE053, Key::Delete),
            (0xE05B, Key::MetaLeft),
            (0xE05C, Key::MetaRight),
            (0xE05D, Key::ContextMenu),
        ];

        for (scan_code, expected) in cases {
            assert_eq!(
                windows_set1_key(scan_code),
                Some(expected),
                "0x{scan_code:04X}"
            );
        }
    }

    #[test]
    fn windows_set1_rejects_unknown_and_invalid_multibyte_codes() {
        for scan_code in [0, 0x54, 0x59, 0xD3, 0xE000, 0xE054, 0xE11D45, 0x0101] {
            assert_eq!(windows_set1_key(scan_code), None, "0x{scan_code:X}");
        }
    }

    #[test]
    fn audited_control_keys_map_without_hid_round_trip() {
        assert_eq!(
            control_key(ControlKey::RWin as i32),
            Some(CanonicalKey::MetaRight)
        );
        assert_eq!(
            control_key(ControlKey::Apps as i32),
            Some(CanonicalKey::ContextMenu)
        );
        assert_eq!(
            control_key(ControlKey::Snapshot as i32),
            Some(CanonicalKey::PrintScreen)
        );
        assert_eq!(control_key(ControlKey::Power as i32), None);
    }
}
