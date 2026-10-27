//! Device-side mouse jiggler. Real input invalidates every remaining movement.

use std::collections::HashSet;
use std::time::{Duration, Instant};

use crate::config::{HidBackend, HidConfig, MouseJigglerConfig};

use super::{CanonicalKey, KeyEventType, KeyboardEvent, MouseEvent, MouseEventType};

const STEP_DELAY: Duration = Duration::from_millis(100);

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Relative,
    Absolute,
}

pub(super) struct JigglerState {
    config: MouseJigglerConfig,
    relative: bool,
    absolute: bool,
    mode: Mode,
    ready: bool,
    activity: Instant,
    keys: HashSet<CanonicalKey>,
    modifiers: u8,
    buttons: u8,
    consumer: u16,
    last_absolute: Option<(i32, i32)>,
    return_move: Option<(MouseEvent, Instant)>,
    generation: u64,
}

impl Default for JigglerState {
    fn default() -> Self {
        Self {
            config: MouseJigglerConfig::default(),
            relative: false,
            absolute: false,
            mode: Mode::Relative,
            ready: false,
            activity: Instant::now(),
            keys: HashSet::new(),
            modifiers: 0,
            buttons: 0,
            consumer: 0,
            last_absolute: None,
            return_move: None,
            generation: 0,
        }
    }
}

impl JigglerState {
    pub fn configure(&mut self, hid: &HidConfig, now: Instant) {
        let (relative, absolute) = match hid.backend {
            HidBackend::None => (false, false),
            HidBackend::Bluetooth => (true, false),
            HidBackend::Otg => {
                let functions = hid.effective_otg_functions();
                (functions.mouse_relative, functions.mouse_absolute)
            }
            _ => (true, true),
        };
        let mode = if absolute && (hid.mouse_absolute || !relative) {
            Mode::Absolute
        } else {
            Mode::Relative
        };
        if self.config != hid.jiggler
            || (self.relative, self.absolute, self.mode) != (relative, absolute, mode)
        {
            self.config = hid.jiggler.clone();
            self.relative = relative;
            self.absolute = absolute;
            self.mode = mode;
            self.activity(now);
        }
    }

    pub fn activity(&mut self, now: Instant) {
        self.activity = now;
        self.return_move = None;
        self.generation = self.generation.wrapping_add(1);
    }

    pub fn keyboard(&mut self, event: &KeyboardEvent, now: Instant) {
        self.activity(now);
        match event.event_type {
            KeyEventType::Down => {
                self.keys.insert(event.key);
            }
            KeyEventType::Up => {
                self.keys.remove(&event.key);
            }
        }
        self.modifiers = event.modifiers.to_hid_byte();
    }

    pub fn mouse(&mut self, event: &MouseEvent, now: Instant) {
        self.activity(now);
        match event.event_type {
            MouseEventType::Move if self.relative => self.mode = Mode::Relative,
            MouseEventType::MoveAbs if self.absolute => {
                self.mode = Mode::Absolute;
                self.last_absolute = Some((event.x.clamp(0, 32767), event.y.clamp(0, 32767)));
            }
            MouseEventType::Down => {
                if let Some(button) = event.button {
                    self.buttons |= button.to_hid_bit();
                }
            }
            MouseEventType::Up => {
                if let Some(button) = event.button {
                    self.buttons &= !button.to_hid_bit();
                }
            }
            _ => {}
        }
    }

    pub fn consumer(&mut self, usage: u16, now: Instant) {
        self.activity(now);
        self.consumer = usage;
    }

    pub fn reset(&mut self, now: Instant) {
        self.activity(now);
        self.keys.clear();
        self.modifiers = 0;
        self.buttons = 0;
        self.consumer = 0;
    }

    pub fn suspend(&mut self) {
        self.ready = false;
        self.last_absolute = None;
        self.reset(Instant::now());
    }

    pub fn delay(&self) -> Option<Duration> {
        (self.config.enabled && (self.relative || self.absolute)).then_some(
            if self.return_move.is_some() {
                STEP_DELAY
            } else {
                Duration::from_secs(1)
            },
        )
    }

    pub fn valid(&self, generation: u64) -> bool {
        self.config.enabled && self.ready && self.generation == generation
    }

    pub fn tick(
        &mut self,
        ready: bool,
        resolution: (u32, u32),
        now: Instant,
    ) -> Option<(MouseEvent, u64)> {
        if ready != self.ready {
            self.ready = ready;
            if ready {
                // Input may already have arrived since reconnecting. Preserve
                // held keys/buttons when starting the new idle period.
                self.activity(now);
            } else {
                self.reset(now);
                self.last_absolute = None;
            }
        }
        if !self.ready
            || !self.config.enabled
            || (!self.relative && !self.absolute)
            || !self.keys.is_empty()
            || self.modifiers != 0
            || self.buttons != 0
            || self.consumer != 0
        {
            return None;
        }
        if let Some((_, deadline)) = &self.return_move {
            if now < *deadline {
                return None;
            }
            let (event, _) = self.return_move.take().unwrap();
            self.activity = now;
            return Some((event, self.generation));
        }
        if now.duration_since(self.activity) < Duration::from_secs(self.config.interval_secs.into())
        {
            return None;
        }
        let (event, back) = if self.mode == Mode::Absolute
            && self.absolute
            && (self.last_absolute.is_some() || !self.relative)
        {
            // An absolute-only device has no way to query the host cursor position.
            let (x, y) = self.last_absolute.unwrap_or((16384, 16384));
            let offset = |position: i32, extent: u32| {
                let delta = (65534 / extent.max(1)).clamp(1, 512) as i32;
                if position + delta <= 32767 {
                    position + delta
                } else {
                    position - delta
                }
            };
            (
                MouseEvent::move_abs(offset(x, resolution.0), offset(y, resolution.1)),
                MouseEvent::move_abs(x, y),
            )
        } else {
            (MouseEvent::move_rel(2, 0), MouseEvent::move_rel(-2, 0))
        };
        self.return_move = Some((back, now + STEP_DELAY));
        Some((event, self.generation))
    }

    pub fn failed(&mut self, generation: u64, now: Instant) {
        if self.generation == generation {
            self.activity(now);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::OtgHidProfile;
    use crate::hid::{KeyboardModifiers, MouseButton};

    fn configured(now: Instant) -> (JigglerState, HidConfig) {
        let mut config = HidConfig::default();
        config.backend = HidBackend::Otg;
        config.jiggler.enabled = true;
        config.jiggler.interval_secs = 10;
        let mut state = JigglerState::default();
        state.configure(&config, now);
        assert!(state.tick(true, (1920, 1080), now).is_none());
        (state, config)
    }

    #[test]
    fn jiggler_waits_then_moves_out_and_back_without_counting_synthetic_input() {
        let now = Instant::now();
        let (mut state, _) = configured(now);
        assert!(state
            .tick(true, (1920, 1080), now + Duration::from_secs(9))
            .is_none());
        let due = now + Duration::from_secs(10);
        let (out, generation) = state.tick(true, (1920, 1080), due).unwrap();
        assert_eq!((out.event_type, out.x, out.y), (MouseEventType::Move, 2, 0));
        assert!(state.valid(generation));
        assert!(state
            .tick(true, (1920, 1080), due + STEP_DELAY / 2)
            .is_none());
        let (back, _) = state.tick(true, (1920, 1080), due + STEP_DELAY).unwrap();
        assert_eq!((back.x, back.y), (-2, 0));
        assert!(state
            .tick(true, (1920, 1080), due + Duration::from_secs(10))
            .is_none());
        assert!(state
            .tick(
                true,
                (1920, 1080),
                due + STEP_DELAY + Duration::from_secs(10)
            )
            .is_some());
    }

    #[test]
    fn jiggler_real_input_cancels_return_and_invalidates_dispatched_movement() {
        let now = Instant::now();
        let (mut state, _) = configured(now);
        state.mouse(&MouseEvent::move_abs(32767, 0), now);
        let due = now + Duration::from_secs(10);
        let (out, generation) = state.tick(true, (1920, 1080), due).unwrap();
        assert!(out.x < 32767 && out.y > 0);
        let input = due + Duration::from_millis(50);
        state.mouse(&MouseEvent::move_abs(12000, 24000), input);
        assert!(!state.valid(generation));
        assert!(state.tick(true, (1920, 1080), due + STEP_DELAY).is_none());
        let (next, _) = state
            .tick(true, (1920, 1080), input + Duration::from_secs(10))
            .unwrap();
        assert!(next.x > 12000 && next.x < 12100 && next.y > 24000 && next.y < 24100);
    }

    #[test]
    fn jiggler_pauses_for_held_keys_modifiers_buttons_and_consumer_controls() {
        let now = Instant::now();
        let (mut state, _) = configured(now);
        let down = KeyboardEvent::key_down(CanonicalKey::KeyA, KeyboardModifiers::default());
        state.keyboard(&down, now);
        assert!(state
            .tick(true, (1920, 1080), now + Duration::from_secs(100))
            .is_none());
        state.keyboard(
            &KeyboardEvent::key_up(
                CanonicalKey::KeyA,
                KeyboardModifiers {
                    left_ctrl: true,
                    ..Default::default()
                },
            ),
            now,
        );
        assert!(state
            .tick(true, (1920, 1080), now + Duration::from_secs(100))
            .is_none());
        state.reset(now);
        state.mouse(&MouseEvent::button_down(MouseButton::Left), now);
        assert!(state
            .tick(true, (1920, 1080), now + Duration::from_secs(100))
            .is_none());
        state.mouse(&MouseEvent::button_up(MouseButton::Left), now);
        state.consumer(0xe9, now);
        assert!(state
            .tick(true, (1920, 1080), now + Duration::from_secs(100))
            .is_none());
        state.consumer(0, now);
        assert!(state
            .tick(true, (1920, 1080), now + Duration::from_secs(10))
            .is_some());
    }

    #[test]
    fn jiggler_config_changes_disconnects_and_shutdown_cancel_and_restart_the_wait() {
        let now = Instant::now();
        let (mut state, mut config) = configured(now);
        let due = now + Duration::from_secs(10);
        let (_, generation) = state.tick(true, (1920, 1080), due).unwrap();
        config.jiggler.enabled = false;
        state.configure(&config, due);
        assert!(!state.valid(generation));
        assert!(state.delay().is_none());
        config.jiggler.enabled = true;
        config.jiggler.interval_secs = 60;
        state.configure(&config, due);
        assert!(state
            .tick(true, (1920, 1080), due + Duration::from_secs(59))
            .is_none());
        assert!(state
            .tick(false, (1920, 1080), due + Duration::from_secs(60))
            .is_none());
        let reconnect = due + Duration::from_secs(100);
        assert!(state.tick(true, (1920, 1080), reconnect).is_none());
        assert!(state
            .tick(true, (1920, 1080), reconnect + Duration::from_secs(60))
            .is_some());
        state.suspend();
        assert!(!state.valid(state.generation));
    }

    #[test]
    fn jiggler_handles_bluetooth_relative_only_absolute_only_and_keyboard_only() {
        let now = Instant::now();
        let (mut state, mut config) = configured(now);
        config.backend = HidBackend::Bluetooth;
        state.configure(&config, now);
        assert_eq!(
            state
                .tick(true, (1920, 1080), now + Duration::from_secs(10))
                .unwrap()
                .0
                .event_type,
            MouseEventType::Move
        );
        config.backend = HidBackend::Otg;
        config.otg_profile = OtgHidProfile::Custom;
        config.otg_functions.mouse_relative = false;
        state.configure(&config, now);
        assert_eq!(
            state
                .tick(true, (1920, 1080), now + Duration::from_secs(10))
                .unwrap()
                .0
                .event_type,
            MouseEventType::MoveAbs
        );
        config.otg_profile = OtgHidProfile::LegacyKeyboard;
        state.configure(&config, now);
        assert!(state.delay().is_none());
        assert!(state
            .tick(true, (1920, 1080), now + Duration::from_secs(100))
            .is_none());
    }

    #[test]
    fn jiggler_failed_moves_wait_a_full_interval_before_retrying() {
        let now = Instant::now();
        let (mut state, _) = configured(now);
        let due = now + Duration::from_secs(10);
        let (_, generation) = state.tick(true, (1920, 1080), due).unwrap();
        state.failed(generation, due);
        assert!(state.tick(true, (1920, 1080), due + STEP_DELAY).is_none());
        assert!(state
            .tick(true, (1920, 1080), due + Duration::from_secs(10))
            .is_some());
    }

    #[test]
    fn jiggler_reconnect_does_not_forget_input_received_before_the_first_tick() {
        let now = Instant::now();
        let (mut state, _) = configured(now);
        state.suspend();
        state.keyboard(
            &KeyboardEvent::key_down(CanonicalKey::KeyA, KeyboardModifiers::default()),
            now,
        );
        assert!(state.tick(true, (1920, 1080), now).is_none());
        assert!(state
            .tick(true, (1920, 1080), now + Duration::from_secs(100))
            .is_none());
        state.keyboard(
            &KeyboardEvent::key_up(CanonicalKey::KeyA, KeyboardModifiers::default()),
            now,
        );
        assert!(state
            .tick(true, (1920, 1080), now + Duration::from_secs(10))
            .is_some());
    }

    struct RecordingBackend {
        mouse: tokio::sync::mpsc::UnboundedSender<MouseEvent>,
        runtime: tokio::sync::watch::Sender<()>,
    }

    #[async_trait::async_trait]
    impl crate::hid::HidBackend for RecordingBackend {
        async fn init(&self) -> crate::error::Result<()> {
            Ok(())
        }
        async fn send_keyboard(&self, _: KeyboardEvent) -> crate::error::Result<()> {
            Ok(())
        }
        async fn send_mouse(&self, event: MouseEvent) -> crate::error::Result<()> {
            self.mouse.send(event).unwrap();
            Ok(())
        }
        async fn reset(&self) -> crate::error::Result<()> {
            Ok(())
        }
        async fn shutdown(&self) -> crate::error::Result<()> {
            Ok(())
        }
        fn runtime_snapshot(&self) -> crate::hid::HidBackendRuntimeSnapshot {
            crate::hid::HidBackendRuntimeSnapshot {
                initialized: true,
                online: true,
                ..Default::default()
            }
        }
        fn subscribe_runtime(&self) -> tokio::sync::watch::Receiver<()> {
            self.runtime.subscribe()
        }
    }

    #[tokio::test]
    async fn jiggler_worker_runs_without_clients_and_confirmed_input_cancels_return() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let backend = std::sync::Arc::new(RecordingBackend {
            mouse: tx,
            runtime: tokio::sync::watch::channel(()).0,
        });
        let controller = crate::hid::test_controller(backend).await;
        let (_, config) = configured(Instant::now());
        controller.configure_jiggler(&config);
        {
            let mut state = controller.jiggler.lock();
            state.ready = true;
            state.activity(Instant::now() - Duration::from_secs(9));
        }
        // Bluetooth publishes status periodically, even with no user input.
        let changed = controller.jiggler_changed.clone();
        let notifications = tokio::spawn(async move {
            loop {
                changed.notify_one();
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        });
        let out = tokio::time::timeout(Duration::from_secs(3), rx.recv())
            .await
            .unwrap()
            .unwrap();
        notifications.abort();
        assert_eq!(out.x, 2);
        controller
            .send_keyboard_confirmed(KeyboardEvent::key_down(
                CanonicalKey::KeyA,
                KeyboardModifiers::default(),
            ))
            .await
            .unwrap();
        assert!(tokio::time::timeout(Duration::from_millis(300), rx.recv())
            .await
            .is_err());
        controller.shutdown().await.unwrap();
    }
}
