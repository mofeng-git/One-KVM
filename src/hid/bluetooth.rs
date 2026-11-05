//! Translate canonical One-KVM input into the native BlueZ peripheral.
use super::{
    backend::{HidBackend, HidBackendRuntimeSnapshot},
    types::{
        ConsumerEvent, KeyEventType, KeyboardEvent, KeyboardReport, MouseEvent, MouseEventType,
    },
};
use crate::{
    config::BluetoothHidConfig,
    error::{AppError, Result},
    events::LedState,
};
use async_trait::async_trait;
use one_kvm_bluetooth_hid::{Action, Config, Peripheral, Report};
use tokio::sync::{watch, Mutex};

fn error(message: String) -> AppError {
    AppError::HidError {
        backend: "bluetooth".into(),
        error_code: "bluetooth_error".into(),
        reason: message,
    }
}
#[derive(Default)]
struct InputState {
    keyboard: KeyboardReport,
    buttons: u8,
    generation: u64,
}
pub struct BluetoothBackend {
    peripheral: Peripheral,
    input: Mutex<InputState>,
    runtime: watch::Sender<()>,
    worker: Mutex<Option<tokio::task::JoinHandle<()>>>,
}
impl BluetoothBackend {
    pub fn new(
        config: BluetoothHidConfig,
        bonds: Option<std::sync::Arc<dyn one_kvm_bluetooth_hid::bonds::BondStore>>,
    ) -> Result<Self> {
        let peripheral = Peripheral::start_with_store(
            Config {
                adapter: config.adapter,
                name: config.name,
                peer: config.peer,
            },
            bonds,
        )
        .map_err(error)?;
        let (runtime, _) = watch::channel(());
        Ok(Self {
            peripheral,
            input: Mutex::new(InputState::default()),
            runtime,
            worker: Mutex::new(None),
        })
    }
    fn check(&self, input: &mut InputState) -> Result<()> {
        let status = self.peripheral.status();
        if input.generation != status.generation || !status.ready {
            *input = InputState {
                generation: status.generation,
                ..Default::default()
            };
        }
        if !status.ready {
            return Err(error(status.error.unwrap_or_else(|| {
                "Pair and connect a computer; HID reports are not ready".into()
            })));
        }
        Ok(())
    }
}
#[async_trait]
impl HidBackend for BluetoothBackend {
    async fn init(&self) -> Result<()> {
        let mut status = self.peripheral.subscribe();
        let runtime = self.runtime.clone();
        *self.worker.lock().await = Some(tokio::spawn(async move {
            let mut last_error = None;
            while status.changed().await.is_ok() {
                let error = status.borrow_and_update().error.clone();
                if error != last_error {
                    if let Some(reason) = &error {
                        tracing::warn!(%reason, "Bluetooth HID unavailable");
                    }
                    last_error = error;
                }
                runtime.send_replace(());
            }
        }));
        Ok(())
    }
    async fn send_keyboard(&self, event: KeyboardEvent) -> Result<()> {
        let mut input = self.input.lock().await;
        self.check(&mut input)?;
        apply_key(&mut input.keyboard, &event);
        let result = self
            .peripheral
            .send(Report::Keyboard, input.keyboard.to_bytes().to_vec())
            .await
            .map_err(error);
        if result.is_err() {
            *input = InputState::default();
        }
        result
    }
    async fn send_mouse(&self, event: MouseEvent) -> Result<()> {
        let mut input = self.input.lock().await;
        self.check(&mut input)?;
        let (mut x, mut y, wheel) = match event.event_type {
            MouseEventType::Move => (event.x, event.y, 0),
            MouseEventType::MoveAbs => {
                return Err(AppError::BadRequest(
                    "Bluetooth HID supports relative mouse only".into(),
                ))
            }
            MouseEventType::Down | MouseEventType::Up => {
                if let Some(button) = event.button {
                    let bit = button.to_hid_bit();
                    if event.event_type == MouseEventType::Down {
                        input.buttons |= bit;
                    } else {
                        input.buttons &= !bit;
                    }
                }
                (0, 0, 0)
            }
            MouseEventType::Scroll => (0, 0, event.scroll),
        };
        // Bound malformed remote input without silently clipping normal relative movements.
        if x.unsigned_abs() > 32767 || y.unsigned_abs() > 32767 {
            return Err(AppError::BadRequest(
                "Relative mouse displacement too large".into(),
            ));
        }
        loop {
            let dx = x.clamp(-127, 127);
            let dy = y.clamp(-127, 127);
            self.peripheral
                .send(
                    Report::Mouse,
                    vec![input.buttons, dx as i8 as u8, dy as i8 as u8, wheel as u8],
                )
                .await
                .map_err(error)?;
            x -= dx;
            y -= dy;
            if x == 0 && y == 0 {
                break;
            }
        }
        Ok(())
    }
    async fn send_consumer(&self, event: ConsumerEvent) -> Result<()> {
        let mut input = self.input.lock().await;
        self.check(&mut input)?;
        if event.usage > 0x3ff {
            return Err(AppError::BadRequest(
                "Consumer usage exceeds Bluetooth report range".into(),
            ));
        }
        self.peripheral
            .send(Report::Consumer, event.usage.to_le_bytes().to_vec())
            .await
            .map_err(error)
    }
    async fn reset(&self) -> Result<()> {
        *self.input.lock().await = InputState::default();
        self.peripheral.action(Action::Reset).await.map_err(error)
    }
    async fn shutdown(&self) -> Result<()> {
        let result = self.peripheral.shutdown().await.map_err(error);
        if let Some(worker) = self.worker.lock().await.take() {
            worker.abort();
        }
        result
    }
    fn runtime_snapshot(&self) -> HidBackendRuntimeSnapshot {
        let state = self.peripheral.status();
        HidBackendRuntimeSnapshot {
            initialized: state.initialized,
            online: state.ready,
            supports_absolute_mouse: false,
            keyboard_leds_enabled: true,
            led_state: LedState {
                num_lock: state.leds & 1 != 0,
                caps_lock: state.leds & 2 != 0,
                scroll_lock: state.leds & 4 != 0,
                compose: state.leds & 8 != 0,
                kana: state.leds & 16 != 0,
            },
            device: Some(state.peer.unwrap_or(state.adapter)),
            screen_resolution: None,
            error_code: state.error.as_ref().map(|_| "bluetooth_error".into()),
            error: state.error,
        }
    }
    fn subscribe_runtime(&self) -> watch::Receiver<()> {
        self.runtime.subscribe()
    }
    async fn bluetooth_status(&self) -> Result<serde_json::Value> {
        serde_json::to_value(self.peripheral.status()).map_err(|e| error(e.to_string()))
    }
    async fn bluetooth_action(&self, action: &str, seconds: u32) -> Result<()> {
        if action == "pair" && !(10..=300).contains(&seconds) {
            return Err(AppError::BadRequest(
                "Pairing window must be 10–300 seconds".into(),
            ));
        }
        let action = match action {
            "pair" => Action::Pair(seconds),
            "close" => Action::ClosePairing,
            "forget" => Action::Forget,
            "disconnect" => Action::Disconnect,
            _ => return Err(AppError::BadRequest("Unknown Bluetooth action".into())),
        };
        self.peripheral.action(action).await.map_err(error)
    }
}
fn apply_key(report: &mut KeyboardReport, event: &KeyboardEvent) {
    if let Some(bit) = event.key.modifier_bit() {
        match event.event_type {
            KeyEventType::Down => report.modifiers |= bit,
            KeyEventType::Up => report.modifiers &= !bit,
        }
    } else {
        report.modifiers = event.modifiers.to_hid_byte();
        let usage = event.key.to_hid_usage();
        match event.event_type {
            KeyEventType::Down if !report.keys.contains(&usage) => {
                report.add_key(usage);
            }
            KeyEventType::Up => report.remove_key(usage),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hid::{CanonicalKey, KeyboardModifiers};
    #[test]
    fn repeated_keydown_does_not_leave_stuck_keys() {
        let mut report = KeyboardReport::default();
        let event = KeyboardEvent {
            key: CanonicalKey::KeyA,
            event_type: KeyEventType::Down,
            modifiers: KeyboardModifiers::default(),
        };
        apply_key(&mut report, &event);
        apply_key(&mut report, &event);
        assert_eq!(report.keys.iter().filter(|&&key| key == 4).count(), 1);
        apply_key(
            &mut report,
            &KeyboardEvent {
                event_type: KeyEventType::Up,
                ..event
            },
        );
        assert_eq!(report.to_bytes(), [0; 8]);
    }
    #[test]
    fn modifier_press_and_release() {
        let mut report = KeyboardReport::default();
        let event = KeyboardEvent {
            key: CanonicalKey::ShiftRight,
            event_type: KeyEventType::Down,
            modifiers: KeyboardModifiers::default(),
        };
        apply_key(&mut report, &event);
        assert_eq!(report.modifiers, 0x20);
        apply_key(
            &mut report,
            &KeyboardEvent {
                event_type: KeyEventType::Up,
                ..event
            },
        );
        assert_eq!(report.modifiers, 0);
    }
}
