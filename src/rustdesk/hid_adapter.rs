use super::keyboard_mapping::{self, CharacterMapping, KeyboardCodeSpace};
use super::protocol::hbb::message::key_event as ke_union;
use super::protocol::{ControlKey, KeyEvent, KeyboardMode, MouseEvent};
use crate::hid::{
    CanonicalKey, KeyEventType, KeyboardEvent, KeyboardModifiers, MouseButton,
    MouseEvent as OneKvmMouseEvent, MouseEventType,
};
use tracing::debug;

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

/// Convert using the code space associated with One-KVM's compatibility platform.
pub fn convert_key_events(event: &KeyEvent) -> Vec<KeyboardEvent> {
    convert_key_events_in_code_space(event, KeyboardCodeSpace::WindowsSet1)
}

pub(super) fn convert_key_events_in_code_space(
    event: &KeyEvent,
    code_space: KeyboardCodeSpace,
) -> Vec<KeyboardEvent> {
    let base_modifiers = if is_modifier_control_key(event) {
        KeyboardModifiers::default()
    } else {
        parse_modifiers(event)
    };

    let Some(mapping) = key_event_to_mapping(event, code_space, base_modifiers) else {
        log_rejected_key_event(event, code_space);
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
        vec![KeyboardEvent {
            event_type: if event.down {
                KeyEventType::Down
            } else {
                KeyEventType::Up
            },
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

fn key_event_to_mapping(
    event: &KeyEvent,
    code_space: KeyboardCodeSpace,
    modifiers: KeyboardModifiers,
) -> Option<KeyMapping> {
    let mode = event.mode.enum_value().ok()?;
    match &event.union {
        Some(ke_union::Union::ControlKey(key)) => {
            plain_mapping(keyboard_mapping::control_key(key.value())?, modifiers)
        }
        Some(ke_union::Union::Unicode(ch)) => character_mapping(*ch, modifiers),
        Some(ke_union::Union::Chr(code)) => match mode {
            KeyboardMode::Map | KeyboardMode::Translate => plain_mapping(
                keyboard_mapping::physical_key(code_space, *code)?,
                modifiers,
            ),
            KeyboardMode::Legacy | KeyboardMode::Auto => legacy_character_mapping(*code, modifiers),
        },
        Some(ke_union::Union::Seq(_)) | Some(ke_union::Union::Win2winHotkey(_)) | None => None,
    }
}

fn plain_mapping(key: CanonicalKey, modifiers: KeyboardModifiers) -> Option<KeyMapping> {
    Some(KeyMapping {
        key,
        modifiers,
        added_shift: false,
    })
}

fn character_mapping(ch: u32, modifiers: KeyboardModifiers) -> Option<KeyMapping> {
    let CharacterMapping { key, needs_shift } = keyboard_mapping::character(ch)?;
    if !needs_shift {
        return plain_mapping(key, modifiers);
    }

    let added_shift = !modifiers.left_shift && !modifiers.right_shift;
    let mut shifted_modifiers = modifiers;
    shifted_modifiers.left_shift = true;
    Some(KeyMapping {
        key,
        modifiers: shifted_modifiers,
        added_shift,
    })
}

fn legacy_character_mapping(ch: u32, modifiers: KeyboardModifiers) -> Option<KeyMapping> {
    let CharacterMapping { key, needs_shift } = keyboard_mapping::character(ch)?;
    // Legacy Chr historically relied on the event's modifier list for uppercase
    // letters, while synthesizing Shift for printable US-layout symbols.
    if needs_shift && !(0x41..=0x5A).contains(&ch) {
        character_mapping(ch, modifiers)
    } else {
        plain_mapping(key, modifiers)
    }
}

fn is_modifier_control_key(event: &KeyEvent) -> bool {
    let Some(ke_union::Union::ControlKey(key)) = &event.union else {
        return false;
    };
    matches!(
        key.enum_value(),
        Ok(ControlKey::Control)
            | Ok(ControlKey::Shift)
            | Ok(ControlKey::Alt)
            | Ok(ControlKey::Meta)
            | Ok(ControlKey::RControl)
            | Ok(ControlKey::RShift)
            | Ok(ControlKey::RAlt)
            | Ok(ControlKey::RWin)
    )
}

fn parse_modifiers(event: &KeyEvent) -> KeyboardModifiers {
    let mut modifiers = KeyboardModifiers::default();
    for modifier in &event.modifiers {
        match modifier.enum_value() {
            Ok(ControlKey::Control) => modifiers.left_ctrl = true,
            Ok(ControlKey::Shift) => modifiers.left_shift = true,
            Ok(ControlKey::Alt) => modifiers.left_alt = true,
            Ok(ControlKey::Meta) => modifiers.left_meta = true,
            Ok(ControlKey::RControl) => modifiers.right_ctrl = true,
            Ok(ControlKey::RShift) => modifiers.right_shift = true,
            Ok(ControlKey::RAlt) => modifiers.right_alt = true,
            Ok(ControlKey::RWin) => modifiers.right_meta = true,
            _ => {}
        }
    }
    modifiers
}

fn log_rejected_key_event(event: &KeyEvent, code_space: KeyboardCodeSpace) {
    let mode = event.mode.value();
    match &event.union {
        Some(ke_union::Union::ControlKey(key)) => debug!(
            mode,
            union = "ControlKey",
            raw = format_args!("0x{:X}", key.value()),
            ?code_space,
            "Dropping unsupported RustDesk keyboard event"
        ),
        Some(ke_union::Union::Chr(code)) => debug!(
            mode,
            union = "Chr",
            raw = format_args!("0x{code:X}"),
            ?code_space,
            "Dropping unsupported RustDesk keyboard event"
        ),
        Some(ke_union::Union::Unicode(code)) => debug!(
            mode,
            union = "Unicode",
            raw = format_args!("0x{code:X}"),
            ?code_space,
            "Dropping unsupported RustDesk keyboard event"
        ),
        Some(ke_union::Union::Seq(seq)) => {
            debug!(mode, union = "Seq", raw = ?seq, ?code_space, "Dropping unsupported RustDesk keyboard event")
        }
        Some(ke_union::Union::Win2winHotkey(code)) => debug!(
            mode,
            union = "Win2winHotkey",
            raw = format_args!("0x{code:X}"),
            ?code_space,
            "Dropping unsupported RustDesk keyboard event"
        ),
        None => debug!(
            mode,
            union = "None",
            ?code_space,
            "Dropping unsupported RustDesk keyboard event"
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use protobuf::EnumOrUnknown;

    fn key_event(mode: KeyboardMode, union: ke_union::Union, down: bool) -> KeyEvent {
        let mut event = KeyEvent::new();
        event.mode = EnumOrUnknown::new(mode);
        event.union = Some(union);
        event.down = down;
        event
    }

    fn map_chr(code: u32, down: bool) -> KeyEvent {
        key_event(KeyboardMode::Map, ke_union::Union::Chr(code), down)
    }

    #[test]
    fn mouse_events_keep_existing_semantics() {
        let mut event = MouseEvent::new();
        event.x = -12;
        event.y = 8;
        event.mask = mouse_type::MOVE_RELATIVE;
        let events = convert_mouse_event(&event, 1920, 1080);
        assert_eq!(events[0].event_type, MouseEventType::Move);
        assert_eq!((events[0].x, events[0].y), (-12, 8));

        event.mask = (mouse_button::LEFT << 3) | mouse_type::DOWN;
        let events = convert_mouse_event(&event, 1920, 1080);
        assert_eq!(events[0].event_type, MouseEventType::Down);
        assert_eq!(events[0].button, Some(MouseButton::Left));
        assert_eq!((events[0].x, events[0].y), (0, 0));
    }

    #[test]
    fn map_delete_generates_only_delete_down_and_up() {
        let down = convert_key_events(&map_chr(0xE053, true));
        let up = convert_key_events(&map_chr(0xE053, false));
        assert_eq!((down.len(), up.len()), (1, 1));
        assert_eq!(
            (down[0].event_type, up[0].event_type),
            (KeyEventType::Down, KeyEventType::Up)
        );
        assert_eq!(
            (down[0].key, up[0].key),
            (CanonicalKey::Delete, CanonicalKey::Delete)
        );
        assert_eq!(
            (down[0].key.to_hid_usage(), up[0].key.to_hid_usage()),
            (0x4C, 0x4C)
        );
    }

    #[test]
    fn map_scan_codes_do_not_become_ascii_digits() {
        let alt = convert_key_events(&map_chr(0x38, true));
        let shift = convert_key_events(&map_chr(0x36, true));
        assert_eq!(alt[0].key, CanonicalKey::AltLeft);
        assert_ne!(alt[0].key, CanonicalKey::Digit8);
        assert_eq!(shift[0].key, CanonicalKey::ShiftRight);
        assert_ne!(shift[0].key, CanonicalKey::Digit6);
    }

    #[test]
    fn legacy_uses_character_semantics_for_same_values() {
        let digit8 = key_event(KeyboardMode::Legacy, ke_union::Union::Chr(0x38), true);
        let digit6 = key_event(KeyboardMode::Legacy, ke_union::Union::Chr(0x36), true);
        assert_eq!(convert_key_events(&digit8)[0].key, CanonicalKey::Digit8);
        assert_eq!(convert_key_events(&digit6)[0].key, CanonicalKey::Digit6);

        let uppercase = key_event(KeyboardMode::Legacy, ke_union::Union::Chr(0x41), true);
        let uppercase = convert_key_events(&uppercase);
        assert_eq!(uppercase[0].key, CanonicalKey::KeyA);
        assert!(!uppercase[0].modifiers.left_shift);
    }

    #[test]
    fn translate_chr_uses_physical_fallback_semantics() {
        let event = key_event(KeyboardMode::Translate, ke_union::Union::Chr(0xE053), true);
        assert_eq!(convert_key_events(&event)[0].key, CanonicalKey::Delete);
    }

    #[test]
    fn unknown_map_codes_never_fall_back() {
        for code in [0, 0x59, 0x61, 0x7F, 0xE054, 0x0101] {
            assert!(
                convert_key_events(&map_chr(code, true)).is_empty(),
                "0x{code:X}"
            );
        }

        let ascii_a_value = convert_key_events(&map_chr(0x41, true));
        assert_eq!(ascii_a_value[0].key, CanonicalKey::F7);
        assert_ne!(ascii_a_value[0].key, CanonicalKey::KeyA);
    }

    #[test]
    fn unicode_and_control_keys_stay_independent() {
        let unicode = key_event(
            KeyboardMode::Map,
            ke_union::Union::Unicode('@' as u32),
            true,
        );
        let control = key_event(
            KeyboardMode::Translate,
            ke_union::Union::ControlKey(EnumOrUnknown::new(ControlKey::Delete)),
            true,
        );
        let unicode = convert_key_events(&unicode);
        assert_eq!(unicode[0].key, CanonicalKey::Digit2);
        assert!(unicode[0].modifiers.left_shift);
        assert_eq!(convert_key_events(&control)[0].key, CanonicalKey::Delete);
    }

    #[test]
    fn press_and_repeated_events_preserve_state_model() {
        let mut press = map_chr(0x1E, false);
        press.press = true;
        press
            .modifiers
            .push(EnumOrUnknown::new(ControlKey::Control));
        let events = convert_key_events(&press);
        assert_eq!(events.len(), 2);
        assert_eq!(
            (events[0].event_type, events[1].event_type),
            (KeyEventType::Down, KeyEventType::Up)
        );
        assert_eq!(
            (events[0].key, events[1].key),
            (CanonicalKey::KeyA, CanonicalKey::KeyA)
        );
        assert!(events.iter().all(|event| event.modifiers.left_ctrl));

        let down = map_chr(0x1E, true);
        assert_eq!(convert_key_events(&down)[0].event_type, KeyEventType::Down);
        assert_eq!(convert_key_events(&down)[0].event_type, KeyEventType::Down);
    }

    #[test]
    fn shifted_press_releases_only_synthetic_shift() {
        let mut event = key_event(
            KeyboardMode::Legacy,
            ke_union::Union::Chr('@' as u32),
            false,
        );
        event.press = true;
        let events = convert_key_events(&event);
        assert!(events[0].modifiers.left_shift);
        assert!(!events[1].modifiers.left_shift);
    }

    #[test]
    fn modifier_control_key_does_not_duplicate_state() {
        let mut event = key_event(
            KeyboardMode::Legacy,
            ke_union::Union::ControlKey(EnumOrUnknown::new(ControlKey::RWin)),
            true,
        );
        event.modifiers.push(EnumOrUnknown::new(ControlKey::RWin));
        let events = convert_key_events(&event);
        assert_eq!(events[0].key, CanonicalKey::MetaRight);
        assert_eq!(events[0].modifiers, KeyboardModifiers::default());
    }

    #[test]
    fn legacy_delete_and_audited_controls_are_mapped() {
        for (control, expected) in [
            (ControlKey::Delete, CanonicalKey::Delete),
            (ControlKey::Snapshot, CanonicalKey::PrintScreen),
            (ControlKey::RWin, CanonicalKey::MetaRight),
            (ControlKey::Apps, CanonicalKey::ContextMenu),
        ] {
            let event = key_event(
                KeyboardMode::Legacy,
                ke_union::Union::ControlKey(EnumOrUnknown::new(control)),
                true,
            );
            assert_eq!(convert_key_events(&event)[0].key, expected);
        }
    }

    #[test]
    fn rejects_unknown_modes_and_unsupported_unions() {
        let mut unknown = map_chr(0x1E, true);
        unknown.mode = EnumOrUnknown::from_i32(99);
        assert!(convert_key_events(&unknown).is_empty());

        let seq = key_event(
            KeyboardMode::Translate,
            ke_union::Union::Seq("a".to_string()),
            true,
        );
        assert!(convert_key_events(&seq).is_empty());

        let mut empty = KeyEvent::new();
        empty.mode = EnumOrUnknown::new(KeyboardMode::Legacy);
        assert!(convert_key_events(&empty).is_empty());
    }
}
