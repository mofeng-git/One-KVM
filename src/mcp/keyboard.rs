use crate::hid::{CanonicalKey, KeyboardModifiers};

pub(super) fn char_to_key(ch: char) -> Option<(CanonicalKey, KeyboardModifiers)> {
    let mut mods = KeyboardModifiers::default();
    let key = match ch {
        'a'..='z' => key_name_to_canonical(&ch.to_string())?,
        'A'..='Z' => {
            mods.left_shift = true;
            key_name_to_canonical(&ch.to_ascii_lowercase().to_string())?
        }
        '0' => CanonicalKey::Digit0,
        '1' => CanonicalKey::Digit1,
        '2' => CanonicalKey::Digit2,
        '3' => CanonicalKey::Digit3,
        '4' => CanonicalKey::Digit4,
        '5' => CanonicalKey::Digit5,
        '6' => CanonicalKey::Digit6,
        '7' => CanonicalKey::Digit7,
        '8' => CanonicalKey::Digit8,
        '9' => CanonicalKey::Digit9,
        ' ' => CanonicalKey::Space,
        '\n' => CanonicalKey::Enter,
        '\t' => CanonicalKey::Tab,
        '-' => CanonicalKey::Minus,
        '_' => {
            mods.left_shift = true;
            CanonicalKey::Minus
        }
        '=' => CanonicalKey::Equal,
        '+' => {
            mods.left_shift = true;
            CanonicalKey::Equal
        }
        '.' => CanonicalKey::Period,
        ',' => CanonicalKey::Comma,
        '/' => CanonicalKey::Slash,
        '?' => {
            mods.left_shift = true;
            CanonicalKey::Slash
        }
        ';' => CanonicalKey::Semicolon,
        ':' => {
            mods.left_shift = true;
            CanonicalKey::Semicolon
        }
        '\'' => CanonicalKey::Quote,
        '"' => {
            mods.left_shift = true;
            CanonicalKey::Quote
        }
        '[' => CanonicalKey::BracketLeft,
        '{' => {
            mods.left_shift = true;
            CanonicalKey::BracketLeft
        }
        ']' => CanonicalKey::BracketRight,
        '}' => {
            mods.left_shift = true;
            CanonicalKey::BracketRight
        }
        '\\' => CanonicalKey::Backslash,
        '|' => {
            mods.left_shift = true;
            CanonicalKey::Backslash
        }
        '`' => CanonicalKey::Backquote,
        '~' => {
            mods.left_shift = true;
            CanonicalKey::Backquote
        }
        '!' => {
            mods.left_shift = true;
            CanonicalKey::Digit1
        }
        '@' => {
            mods.left_shift = true;
            CanonicalKey::Digit2
        }
        '#' => {
            mods.left_shift = true;
            CanonicalKey::Digit3
        }
        '$' => {
            mods.left_shift = true;
            CanonicalKey::Digit4
        }
        '%' => {
            mods.left_shift = true;
            CanonicalKey::Digit5
        }
        '^' => {
            mods.left_shift = true;
            CanonicalKey::Digit6
        }
        '&' => {
            mods.left_shift = true;
            CanonicalKey::Digit7
        }
        '*' => {
            mods.left_shift = true;
            CanonicalKey::Digit8
        }
        '(' => {
            mods.left_shift = true;
            CanonicalKey::Digit9
        }
        ')' => {
            mods.left_shift = true;
            CanonicalKey::Digit0
        }
        _ => return None,
    };
    Some((key, mods))
}

pub(super) fn key_name_to_canonical(name: &str) -> Option<CanonicalKey> {
    match name.trim().to_lowercase().as_str() {
        "a" => Some(CanonicalKey::KeyA),
        "b" => Some(CanonicalKey::KeyB),
        "c" => Some(CanonicalKey::KeyC),
        "d" => Some(CanonicalKey::KeyD),
        "e" => Some(CanonicalKey::KeyE),
        "f" => Some(CanonicalKey::KeyF),
        "g" => Some(CanonicalKey::KeyG),
        "h" => Some(CanonicalKey::KeyH),
        "i" => Some(CanonicalKey::KeyI),
        "j" => Some(CanonicalKey::KeyJ),
        "k" => Some(CanonicalKey::KeyK),
        "l" => Some(CanonicalKey::KeyL),
        "m" => Some(CanonicalKey::KeyM),
        "n" => Some(CanonicalKey::KeyN),
        "o" => Some(CanonicalKey::KeyO),
        "p" => Some(CanonicalKey::KeyP),
        "q" => Some(CanonicalKey::KeyQ),
        "r" => Some(CanonicalKey::KeyR),
        "s" => Some(CanonicalKey::KeyS),
        "t" => Some(CanonicalKey::KeyT),
        "u" => Some(CanonicalKey::KeyU),
        "v" => Some(CanonicalKey::KeyV),
        "w" => Some(CanonicalKey::KeyW),
        "x" => Some(CanonicalKey::KeyX),
        "y" => Some(CanonicalKey::KeyY),
        "z" => Some(CanonicalKey::KeyZ),
        "0" => Some(CanonicalKey::Digit0),
        "1" => Some(CanonicalKey::Digit1),
        "2" => Some(CanonicalKey::Digit2),
        "3" => Some(CanonicalKey::Digit3),
        "4" => Some(CanonicalKey::Digit4),
        "5" => Some(CanonicalKey::Digit5),
        "6" => Some(CanonicalKey::Digit6),
        "7" => Some(CanonicalKey::Digit7),
        "8" => Some(CanonicalKey::Digit8),
        "9" => Some(CanonicalKey::Digit9),
        "insert" => Some(CanonicalKey::Insert),
        "capslock" => Some(CanonicalKey::CapsLock),
        "printscreen" => Some(CanonicalKey::PrintScreen),
        "enter" | "return" => Some(CanonicalKey::Enter),
        "escape" | "esc" => Some(CanonicalKey::Escape),
        "backspace" => Some(CanonicalKey::Backspace),
        "tab" => Some(CanonicalKey::Tab),
        "space" => Some(CanonicalKey::Space),
        "delete" | "del" => Some(CanonicalKey::Delete),
        "arrowup" | "up" => Some(CanonicalKey::ArrowUp),
        "arrowdown" | "down" => Some(CanonicalKey::ArrowDown),
        "arrowleft" | "left" => Some(CanonicalKey::ArrowLeft),
        "arrowright" | "right" => Some(CanonicalKey::ArrowRight),
        "home" => Some(CanonicalKey::Home),
        "end" => Some(CanonicalKey::End),
        "pageup" => Some(CanonicalKey::PageUp),
        "pagedown" => Some(CanonicalKey::PageDown),
        "f1" => Some(CanonicalKey::F1),
        "f2" => Some(CanonicalKey::F2),
        "f3" => Some(CanonicalKey::F3),
        "f4" => Some(CanonicalKey::F4),
        "f5" => Some(CanonicalKey::F5),
        "f6" => Some(CanonicalKey::F6),
        "f7" => Some(CanonicalKey::F7),
        "f8" => Some(CanonicalKey::F8),
        "f9" => Some(CanonicalKey::F9),
        "f10" => Some(CanonicalKey::F10),
        "f11" => Some(CanonicalKey::F11),
        "f12" => Some(CanonicalKey::F12),
        _ => None,
    }
}
