//! Shared semantic state, before OTG/serial output selection and coordinate scaling.
//! Callers serialize apply + write/enqueue with the same per-backend mutex.
use super::{MouseEvent, MouseEventType};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum IntentKind {
    RelativeMove,
    AbsoluteMove,
    Buttons,
    Wheel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct MouseIntent {
    pub kind: IntentKind,
    pub buttons: u8,
    /// Canonical 0..32767 coordinates; serial scaling belongs to its adapter.
    pub absolute: (u16, u16),
    pub relative: (i8, i8),
    pub wheel: i8,
}

#[derive(Debug, Default, Clone)]
pub(super) struct MouseState {
    buttons: u8,
    absolute: (u16, u16),
    relative_input: bool,
}

impl MouseState {
    pub fn buttons(&self) -> u8 {
        self.buttons
    }
    pub fn absolute(&self) -> (u16, u16) {
        self.absolute
    }
    pub fn relative_input(&self) -> bool {
        self.relative_input
    }

    pub fn apply(&mut self, event: &MouseEvent) -> Option<MouseIntent> {
        let mut relative = (0, 0);
        let mut wheel = 0;
        let kind = match event.event_type {
            MouseEventType::Move => {
                self.relative_input = true;
                relative = (
                    event.x.clamp(-127, 127) as i8,
                    event.y.clamp(-127, 127) as i8,
                );
                IntentKind::RelativeMove
            }
            MouseEventType::MoveAbs => {
                self.relative_input = false;
                self.absolute = (
                    event.x.clamp(0, 32767) as u16,
                    event.y.clamp(0, 32767) as u16,
                );
                IntentKind::AbsoluteMove
            }
            MouseEventType::Down => {
                self.buttons |= event.button?.to_hid_bit();
                IntentKind::Buttons
            }
            MouseEventType::Up => {
                self.buttons &= !event.button?.to_hid_bit();
                IntentKind::Buttons
            }
            MouseEventType::Scroll => {
                wheel = event.scroll;
                IntentKind::Wheel
            }
        };
        Some(MouseIntent {
            kind,
            buttons: self.buttons,
            absolute: self.absolute,
            relative,
            wheel,
        })
    }

    /// A release does not choose an output or move the pointer. Adapters decide
    /// whether reset retains coordinates (absolute-only OTG) or returns to origin.
    pub fn release_buttons(&mut self) -> MouseIntent {
        self.buttons = 0;
        MouseIntent {
            kind: IntentKind::Buttons,
            buttons: 0,
            absolute: self.absolute,
            relative: (0, 0),
            wheel: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hid::MouseButton;

    #[test]
    fn down_up_wheel_use_canonical_last_position() {
        let mut state = MouseState::default();
        state.apply(&MouseEvent::move_abs(18022, 16384));
        for (event, buttons, wheel) in [
            (MouseEvent::button_down(MouseButton::Left), 1, 0),
            (MouseEvent::scroll(-1), 1, -1),
            (MouseEvent::button_up(MouseButton::Left), 0, 0),
        ] {
            let intent = state.apply(&event).unwrap();
            assert_eq!(
                (intent.buttons, intent.absolute, intent.wheel),
                (buttons, (18022, 16384), wheel)
            );
        }
    }

    #[test]
    fn movement_retains_chord_and_release_retains_position() {
        let mut state = MouseState::default();
        state.apply(&MouseEvent::button_down(MouseButton::Left));
        state.apply(&MouseEvent::button_down(MouseButton::Right));
        assert_eq!(
            state
                .apply(&MouseEvent::move_abs(123, 456))
                .unwrap()
                .buttons,
            3
        );
        assert_eq!(
            state
                .apply(&MouseEvent::button_up(MouseButton::Left))
                .unwrap()
                .buttons,
            2
        );
        let released = state.release_buttons();
        assert_eq!((released.buttons, released.absolute), (0, (123, 456)));
        assert_eq!(state.apply(&MouseEvent::move_rel(5, 6)).unwrap().buttons, 0);
    }

    #[test]
    fn normalization_and_input_mode_do_not_select_transport() {
        let mut state = MouseState::default();
        assert_eq!(
            state
                .apply(&MouseEvent::move_abs(-1, 99999))
                .unwrap()
                .absolute,
            (0, 32767)
        );
        assert!(!state.relative_input());
        let relative = state.apply(&MouseEvent::move_rel(-999, 999)).unwrap();
        assert_eq!(relative.relative, (-127, 127));
        assert_eq!(relative.absolute, (0, 32767));
        assert!(state.relative_input());
        assert_eq!(state.apply(&MouseEvent::scroll(-128)).unwrap().wheel, -128);
    }

    #[test]
    fn invalid_button_event_does_not_change_state() {
        let mut state = MouseState::default();
        state.apply(&MouseEvent::button_down(MouseButton::Left));
        let mut invalid = MouseEvent::button_up(MouseButton::Left);
        invalid.button = None;
        assert!(state.apply(&invalid).is_none());
        assert_eq!(state.buttons(), 1);
    }

    #[test]
    fn backend_states_and_reset_policies_are_independent() {
        let mut first = MouseState::default();
        let second = MouseState::default();
        first.apply(&MouseEvent::move_abs(12000, 16000));
        first.apply(&MouseEvent::button_down(MouseButton::Left));
        assert_eq!((second.buttons(), second.absolute()), (0, (0, 0)));
        first.release_buttons();
        assert_eq!(first.absolute(), (12000, 16000));
        first = MouseState::default();
        assert_eq!(
            (first.buttons(), first.absolute(), first.relative_input()),
            (0, (0, 0), false)
        );
    }
}
