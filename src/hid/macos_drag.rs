//! Experimental absolute-button / relative-drag routing shared by USB backends.
//! Relative counts are subject to host acceleration; they are not screen pixels.

use super::{MouseEvent, MouseEventType};

#[derive(Debug, PartialEq)]
pub(super) enum MouseReport {
    Absolute {
        buttons: u8,
        x: u16,
        y: u16,
    },
    Relative {
        buttons: u8,
        dx: i8,
        dy: i8,
        wheel: i8,
    },
}

#[derive(Default)]
pub(super) struct MacosDrag {
    position: (u16, u16),
    remainder: (i64, i64),
    extent: (u32, u32),
    relative_input: bool,
    absolute_drag: bool,
}

impl MacosDrag {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn plan(
        &mut self,
        event: MouseEvent,
        buttons: u8,
        extent: (u32, u32),
    ) -> (u8, Vec<MouseReport>) {
        let mut reports = Vec::new();
        let mut next_buttons = buttons;
        if extent != self.extent || buttons == 0 {
            self.remainder = (0, 0);
            self.extent = extent;
        }
        match event.event_type {
            MouseEventType::MoveAbs => {
                let position = (
                    event.x.clamp(0, 32767) as u16,
                    event.y.clamp(0, 32767) as u16,
                );
                self.relative_input = false;
                if buttons != 0 {
                    let dx =
                        Self::delta(position.0, self.position.0, extent.0, &mut self.remainder.0);
                    let dy =
                        Self::delta(position.1, self.position.1, extent.1, &mut self.remainder.1);
                    Self::motion(&mut reports, buttons, dx, dy);
                } else {
                    reports.push(MouseReport::Absolute {
                        buttons: 0,
                        x: position.0,
                        y: position.1,
                    });
                }
                self.position = position;
            }
            MouseEventType::Move => {
                self.relative_input = true;
                Self::motion(
                    &mut reports,
                    buttons,
                    i64::from(event.x.clamp(-127, 127)),
                    i64::from(event.y.clamp(-127, 127)),
                );
            }
            MouseEventType::Down | MouseEventType::Up => {
                if let Some(button) = event.button {
                    let down = event.event_type == MouseEventType::Down;
                    next_buttons = if down {
                        buttons | button.to_hid_bit()
                    } else {
                        buttons & !button.to_hid_bit()
                    };
                    if down && buttons == 0 {
                        self.absolute_drag = !self.relative_input;
                    }
                    // Latch the button route until all buttons are released, even
                    // if the client changes pointer mode during the drag.
                    if self.absolute_drag {
                        if !down {
                            // Relative drag reports also carry buttons. Clear their
                            // state before releasing the absolute collection.
                            reports.push(MouseReport::Relative {
                                buttons: next_buttons,
                                dx: 0,
                                dy: 0,
                                wheel: 0,
                            });
                        }
                        reports.push(MouseReport::Absolute {
                            buttons: next_buttons,
                            x: self.position.0,
                            y: self.position.1,
                        });
                    } else {
                        reports.push(MouseReport::Relative {
                            buttons: next_buttons,
                            dx: 0,
                            dy: 0,
                            wheel: 0,
                        });
                    }
                    if next_buttons == 0 {
                        self.absolute_drag = false;
                        self.remainder = (0, 0);
                    }
                }
            }
            MouseEventType::Scroll => {
                reports.push(MouseReport::Relative {
                    buttons,
                    dx: 0,
                    dy: 0,
                    wheel: event.scroll,
                });
            }
        }
        (next_buttons, reports)
    }

    fn delta(current: u16, previous: u16, extent: u32, remainder: &mut i64) -> i64 {
        // Keep the original 15-bit input precision and carry fractional counts.
        let scaled =
            (i64::from(current) - i64::from(previous)) * i64::from(extent.max(1)) + *remainder;
        *remainder = scaled % 32768;
        scaled / 32768
    }

    fn motion(reports: &mut Vec<MouseReport>, buttons: u8, dx: i64, dy: i64) {
        // Distribute both axes over the same packets to preserve diagonal paths.
        let count = (dx.abs().max(dy.abs()) + 126) / 127;
        let mut previous = (0, 0);
        for index in 1..=count {
            let position = (dx * index / count, dy * index / count);
            reports.push(MouseReport::Relative {
                buttons,
                dx: (position.0 - previous.0) as i8,
                dy: (position.1 - previous.1) as i8,
                wheel: 0,
            });
            previous = position;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hid::MouseButton;

    fn drag(steps: &[i32], extent: (u32, u32)) -> (i64, i64) {
        let mut state = MacosDrag::default();
        state.plan(MouseEvent::move_abs(8000, 8000), 0, extent);
        state.plan(MouseEvent::button_down(MouseButton::Left), 0, extent);
        let mut total = (0, 0);
        for &x in steps {
            for report in state.plan(MouseEvent::move_abs(x, x), 1, extent).1 {
                if let MouseReport::Relative { dx, dy, .. } = report {
                    total.0 += i64::from(dx);
                    total.1 += i64::from(dy);
                } else {
                    panic!("absolute movement during drag");
                }
            }
        }
        total
    }

    #[test]
    fn preserves_total_across_event_splitting_and_large_moves() {
        let steps: Vec<_> = (8001..=16000).collect();
        assert_eq!(drag(&[16000], (1920, 1080)), (468, 263));
        assert_eq!(drag(&steps, (1920, 1080)), (468, 263));
        assert_eq!(drag(&[16000, 8000], (1920, 1080)), (0, 0));
        assert_eq!(drag(&[16000], (3840, 2160)), (937, 527));
    }

    #[test]
    fn scroll_and_mode_switch_keep_absolute_drag_release() {
        let mut state = MacosDrag::default();
        let extent = (1920, 1080);
        state.plan(MouseEvent::move_abs(8000, 8000), 0, extent);
        state.plan(MouseEvent::button_down(MouseButton::Left), 0, extent);
        let mut wheel = MouseEvent::move_abs(0, 0);
        wheel.event_type = MouseEventType::Scroll;
        wheel.scroll = 1;
        assert_eq!(
            state.plan(wheel, 1, extent).1,
            vec![MouseReport::Relative {
                buttons: 1,
                dx: 0,
                dy: 0,
                wheel: 1
            }]
        );
        state.plan(MouseEvent::move_rel(10, 0), 1, extent);
        assert_eq!(
            state
                .plan(MouseEvent::button_up(MouseButton::Left), 1, extent)
                .1,
            vec![
                MouseReport::Relative {
                    buttons: 0,
                    dx: 0,
                    dy: 0,
                    wheel: 0
                },
                MouseReport::Absolute {
                    buttons: 0,
                    x: 8000,
                    y: 8000
                },
            ]
        );
    }

    #[test]
    fn native_relative_clicks_and_reset() {
        let mut state = MacosDrag::default();
        state.plan(MouseEvent::move_rel(1, 1), 0, (1920, 1080));
        assert_eq!(
            state
                .plan(MouseEvent::button_down(MouseButton::Left), 0, (1920, 1080))
                .1,
            vec![MouseReport::Relative {
                buttons: 1,
                dx: 0,
                dy: 0,
                wheel: 0
            }]
        );
        state.reset();
        assert_eq!(
            state
                .plan(MouseEvent::button_down(MouseButton::Left), 0, (1920, 1080))
                .1,
            vec![MouseReport::Absolute {
                buttons: 1,
                x: 0,
                y: 0
            }]
        );
    }
}
