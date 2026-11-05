use super::keyboard::{char_to_key, key_name_to_canonical};
use crate::hid::{
    CanonicalKey, HidController, KeyEventType, KeyboardEvent, KeyboardModifiers, MouseButton,
    MouseEvent,
};
use rmcp::schemars::{self, JsonSchema};
use serde::Deserialize;
use std::time::Duration;

#[derive(Debug, Clone, Copy, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "snake_case")]
pub enum Button {
    #[default]
    Left,
    Middle,
    Right,
}
impl From<Button> for MouseButton {
    fn from(b: Button) -> Self {
        match b {
            Button::Left => Self::Left,
            Button::Middle => Self::Middle,
            Button::Right => Self::Right,
        }
    }
}
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    Click {
        x: f64,
        y: f64,
        #[serde(default)]
        button: Button,
    },
    DoubleClick {
        x: f64,
        y: f64,
        #[serde(default)]
        button: Button,
    },
    Move {
        x: f64,
        y: f64,
    },
    Drag {
        path: Vec<Point>,
        #[serde(default)]
        button: Button,
    },
    Scroll {
        x: f64,
        y: f64,
        vertical_steps: i32,
    },
    Type {
        text: String,
    },
    Keypress {
        /// Modifiers (ctrl, shift, alt, meta) plus one key: a-z, 0-9, enter, esc, tab, space, backspace, delete, arrows, home/end, pageup/pagedown, f1-f12.
        keys: Vec<String>,
        #[serde(default = "default_hold")]
        hold_ms: u64,
    },
    Wait {
        ms: u64,
    },
}
fn default_hold() -> u64 {
    35
}
#[derive(Debug)]
pub(super) enum Step {
    Mouse(MouseEvent),
    Keyboard(KeyboardEvent),
    Pause(u64),
}
#[derive(Debug)]
pub(super) struct PreparedActions {
    pub actions: Vec<Vec<Step>>,
    pub needs_absolute_mouse: bool,
}

fn position(x: f64, y: f64) -> Result<Step, String> {
    if !x.is_finite() || !y.is_finite() || !(0.0..=1.0).contains(&x) || !(0.0..=1.0).contains(&y) {
        return Err("Mouse coordinates must be finite numbers in [0,1]".into());
    }
    Ok(Step::Mouse(MouseEvent::move_abs(
        (x * 32767.0).round() as i32,
        (y * 32767.0).round() as i32,
    )))
}
fn key_steps(out: &mut Vec<Step>, key: CanonicalKey, modifiers: KeyboardModifiers, hold: u64) {
    out.push(Step::Keyboard(KeyboardEvent {
        event_type: KeyEventType::Down,
        key,
        modifiers,
    }));
    out.push(Step::Pause(hold));
    out.push(Step::Keyboard(KeyboardEvent {
        event_type: KeyEventType::Up,
        key,
        modifiers: KeyboardModifiers::default(),
    }));
}
fn chord(keys: &[String]) -> Result<(CanonicalKey, KeyboardModifiers), String> {
    if keys.is_empty() || keys.len() > 5 {
        return Err("keypress requires modifiers and one ordinary key".into());
    }
    let mut mods = KeyboardModifiers::default();
    let mut key = None;
    let mut modifier_key = None;
    for name in keys {
        let modifier = match name.trim().to_lowercase().as_str() {
            "ctrl" | "control" | "controlleft" => {
                mods.left_ctrl = true;
                Some(CanonicalKey::ControlLeft)
            }
            "shift" | "shiftleft" => {
                mods.left_shift = true;
                Some(CanonicalKey::ShiftLeft)
            }
            "alt" | "altleft" => {
                mods.left_alt = true;
                Some(CanonicalKey::AltLeft)
            }
            "meta" | "win" | "cmd" | "super" | "metaleft" => {
                mods.left_meta = true;
                Some(CanonicalKey::MetaLeft)
            }
            "controlright" => {
                mods.right_ctrl = true;
                Some(CanonicalKey::ControlRight)
            }
            "shiftright" => {
                mods.right_shift = true;
                Some(CanonicalKey::ShiftRight)
            }
            "altright" | "altgr" => {
                mods.right_alt = true;
                Some(CanonicalKey::AltRight)
            }
            "metaright" => {
                mods.right_meta = true;
                Some(CanonicalKey::MetaRight)
            }
            _ => None,
        };
        if let Some(modifier) = modifier {
            modifier_key = Some(modifier);
            continue;
        }
        let parsed = key_name_to_canonical(name)
            .or_else(|| {
                let mut chars = name.chars();
                let ch = chars.next()?;
                if chars.next().is_some() {
                    return None;
                }
                let (key, implied) = char_to_key(ch)?;
                mods.left_shift |= implied.left_shift;
                Some(key)
            })
            .ok_or("Unsupported key name")?;
        if key.replace(parsed).is_some() {
            return Err("Only one ordinary key per keypress is supported".into());
        }
    }
    Ok((key.or(modifier_key).ok_or("No key provided")?, mods))
}

pub(super) fn prepare(actions: Vec<Action>) -> Result<PreparedActions, String> {
    if actions.is_empty() || actions.len() > 32 {
        return Err("Provide between 1 and 32 actions".into());
    }
    let mut prepared = PreparedActions {
        actions: Vec::new(),
        needs_absolute_mouse: false,
    };
    let mut characters = 0;
    let mut duration = 0u64;
    for action in actions {
        let mut steps = Vec::new();
        match action {
            Action::Click { x, y, button } | Action::DoubleClick { x, y, button } => {
                let count = if matches!(action, Action::DoubleClick { .. }) {
                    2
                } else {
                    1
                };
                steps.push(position(x, y)?);
                for _ in 0..count {
                    steps.push(Step::Mouse(MouseEvent::button_down(button.into())));
                    steps.push(Step::Pause(35));
                    steps.push(Step::Mouse(MouseEvent::button_up(button.into())));
                    steps.push(Step::Pause(35));
                }
                prepared.needs_absolute_mouse = true;
            }
            Action::Move { x, y } => {
                steps.push(position(x, y)?);
                prepared.needs_absolute_mouse = true;
            }
            Action::Drag { path, button } => {
                if path.len() < 2 || path.len() > 128 {
                    return Err("Drag path needs 2 to 128 points".into());
                }
                for (i, p) in path.into_iter().enumerate() {
                    steps.push(position(p.x, p.y)?);
                    if i == 0 {
                        steps.push(Step::Mouse(MouseEvent::button_down(button.into())));
                    }
                    steps.push(Step::Pause(35));
                }
                steps.push(Step::Mouse(MouseEvent::button_up(button.into())));
                prepared.needs_absolute_mouse = true;
            }
            Action::Scroll {
                x,
                y,
                vertical_steps,
            } => {
                if !(-100..=100).contains(&vertical_steps) {
                    return Err("vertical_steps must be between -100 and 100".into());
                }
                steps.push(position(x, y)?);
                for _ in 0..vertical_steps.unsigned_abs() {
                    // The device wheel convention is positive up; public MCP is positive down.
                    steps.push(Step::Mouse(MouseEvent::scroll(
                        -vertical_steps.signum() as i8
                    )));
                    steps.push(Step::Pause(35));
                }
                prepared.needs_absolute_mouse = true;
            }
            Action::Type { text } => {
                characters += text.chars().count();
                if characters > 256 {
                    return Err("At most 256 characters per batch".into());
                }
                for ch in text.chars() {
                    let (key, mods) = char_to_key(ch).ok_or(
                        "Text must use the US keyboard ASCII mapping (including newline and tab)",
                    )?;
                    key_steps(&mut steps, key, mods, 35);
                }
            }
            Action::Keypress { keys, hold_ms } => {
                if hold_ms > 5000 {
                    return Err("hold_ms must not exceed 5000".into());
                }
                let (key, mods) = chord(&keys)?;
                key_steps(&mut steps, key, mods, hold_ms);
            }
            Action::Wait { ms } => {
                if ms > 5000 {
                    return Err("wait ms must not exceed 5000".into());
                }
                steps.push(Step::Pause(ms));
            }
        }
        steps.push(Step::Pause(120));
        duration += steps
            .iter()
            .map(|s| match s {
                Step::Pause(ms) => *ms,
                _ => 1,
            })
            .sum::<u64>();
        if duration > 25_000 {
            return Err("Batch duration exceeds 25 seconds before hardware delays".into());
        }
        prepared.actions.push(steps);
    }
    Ok(prepared)
}

pub(super) async fn execute(hid: &HidController, steps: Vec<Step>) -> crate::Result<()> {
    for step in steps {
        match step {
            Step::Mouse(event) => hid.send_mouse_confirmed(event).await?,
            Step::Keyboard(event) => hid.send_keyboard_confirmed(event).await?,
            Step::Pause(ms) => tokio::time::sleep(Duration::from_millis(ms)).await,
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validates_entire_batch_before_any_hid_input() {
        assert!(prepare(vec![
            Action::Type {
                text: "valid".into()
            },
            Action::Type {
                text: "中文".into()
            }
        ])
        .is_err());
        assert!(prepare(vec![Action::Move {
            x: f64::NAN,
            y: 0.5
        }])
        .is_err());
        assert!(prepare(vec![Action::Move { x: 1.01, y: 0.5 }]).is_err());
        assert!(chord(&["ctrl".into(), "unknown".into()]).is_err());
        assert!(chord(&["a".into(), "b".into()]).is_err());
        assert!(chord(&["ctrl".into(), "alt".into(), "delete".into()]).is_ok());
    }
    #[test]
    fn uses_full_hid_range_and_bounded_waits() {
        let actions = prepare(vec![Action::Move { x: 1.0, y: 0.0 }]).unwrap();
        match &actions.actions[0][0] {
            Step::Mouse(event) => assert_eq!((event.x, event.y), (32767, 0)),
            _ => panic!(),
        }
        assert!(prepare((0..6).map(|_| Action::Wait { ms: 5000 }).collect()).is_err());
        assert!(prepare(vec![Action::Type {
            text: "x".repeat(257)
        }])
        .is_err());
    }
}
