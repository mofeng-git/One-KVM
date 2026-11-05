//! Audio device-loss monitoring and serialized recovery.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use tokio::sync::{Mutex, RwLock};
use tracing::{debug, info, warn};

use super::capture::AudioConfig;
use super::controller::AudioRecoveredCallback;
use super::device::{enumerate_audio_devices, AudioDeviceInfo};
use super::monitor::AudioHealthMonitor;
use super::streamer::{AudioStreamState, AudioStreamer, AudioStreamerConfig};
use super::types::AudioControllerConfig;
use crate::events::{EventBus, StreamKind, SystemEvent};

const RETRY_DELAY: std::time::Duration = std::time::Duration::from_secs(1);

struct RecoveryControl {
    /// Even values are idle; the following odd value is that recovery's token.
    /// A single compare-exchange therefore owns both activity and generation.
    state: AtomicU64,
}

impl RecoveryControl {
    fn new() -> Self {
        Self {
            state: AtomicU64::new(0),
        }
    }

    fn begin(&self) -> Option<u64> {
        let idle = self.state.load(Ordering::Acquire);
        if !idle.is_multiple_of(2) {
            return None;
        }
        let token = idle.wrapping_add(1);
        self.state
            .compare_exchange(idle, token, Ordering::AcqRel, Ordering::Acquire)
            .ok()
            .map(|_| token)
    }

    fn is_current(&self, token: u64) -> bool {
        self.state.load(Ordering::Acquire) == token
    }

    fn finish(&self, token: u64) {
        let _ = self.state.compare_exchange(
            token,
            token.wrapping_add(1),
            Ordering::AcqRel,
            Ordering::Acquire,
        );
    }

    fn cancel(&self) {
        let token = self.state.load(Ordering::Acquire);
        if !token.is_multiple_of(2) {
            let _ = self.state.compare_exchange(
                token,
                token.wrapping_add(1),
                Ordering::AcqRel,
                Ordering::Acquire,
            );
        }
    }
}

struct RecoveryLease {
    control: Arc<RecoveryControl>,
    generation: u64,
}

impl Drop for RecoveryLease {
    fn drop(&mut self) {
        self.control.finish(self.generation);
    }
}

struct RecoveryInner {
    config: Arc<RwLock<AudioControllerConfig>>,
    streamer: Arc<RwLock<Option<Arc<AudioStreamer>>>>,
    event_bus: Arc<RwLock<Option<Arc<EventBus>>>>,
    monitor: Arc<AudioHealthMonitor>,
    recovered_callback: Arc<RwLock<Option<AudioRecoveredCallback>>>,
    operation: Arc<Mutex<()>>,
    control: Arc<RecoveryControl>,
}

#[derive(Clone)]
pub(super) struct AudioRecovery {
    inner: Arc<RecoveryInner>,
}

impl AudioRecovery {
    pub(super) fn new(
        config: Arc<RwLock<AudioControllerConfig>>,
        streamer: Arc<RwLock<Option<Arc<AudioStreamer>>>>,
        event_bus: Arc<RwLock<Option<Arc<EventBus>>>>,
        monitor: Arc<AudioHealthMonitor>,
        recovered_callback: Arc<RwLock<Option<AudioRecoveredCallback>>>,
        operation: Arc<Mutex<()>>,
    ) -> Self {
        Self {
            inner: Arc::new(RecoveryInner {
                config,
                streamer,
                event_bus,
                monitor,
                recovered_callback,
                operation,
                control: Arc::new(RecoveryControl::new()),
            }),
        }
    }

    pub(super) fn cancel(&self) {
        self.inner.control.cancel();
    }

    pub(super) fn monitor(&self, streamer: Arc<AudioStreamer>, device: String) {
        let recovery = self.clone();
        let mut state = streamer.state_watch();
        tokio::spawn(async move {
            loop {
                let current_state = *state.borrow();
                match current_state {
                    AudioStreamState::Error => {}
                    AudioStreamState::Stopped => return,
                    AudioStreamState::Starting | AudioStreamState::Running => {
                        if state.changed().await.is_err() {
                            return;
                        }
                        continue;
                    }
                }

                // Serialize the ownership check with user-driven start/stop
                // operations. If a stop already owns the operation lock, it
                // removes the streamer before this monitor may start recovery.
                let _operation = recovery.inner.operation.lock().await;
                let is_current = recovery
                    .inner
                    .streamer
                    .read()
                    .await
                    .as_ref()
                    .is_some_and(|current| Arc::ptr_eq(current, &streamer));
                if !is_current {
                    return;
                }

                let reason = format!("Audio device lost: {device}");
                recovery
                    .inner
                    .monitor
                    .report_error(&reason, "device_lost")
                    .await;
                recovery.start(device, reason);
                return;
            }
        });
    }

    pub(super) fn start(&self, lost_device: String, reason: String) {
        let Some(generation) = self.inner.control.begin() else {
            debug!("Audio recovery already in progress");
            return;
        };
        let recovery = self.clone();
        tokio::spawn(async move {
            let _lease = RecoveryLease {
                control: recovery.inner.control.clone(),
                generation,
            };
            recovery.run(generation, lost_device, reason).await;
        });
    }

    async fn run(&self, generation: u64, lost_device: String, reason: String) {
        warn!("Audio recovery started for {lost_device}: {reason}");
        self.publish_device_lost(&lost_device, &reason).await;
        self.publish_state(
            "device_lost",
            Some(lost_device.clone()),
            Some("audio_device_lost"),
            Some(RETRY_DELAY.as_millis() as u64),
        )
        .await;

        let mut attempt = 0u32;
        while self.inner.control.is_current(generation) {
            let config = self.inner.config.read().await.clone();
            if !config.enabled {
                return;
            }
            if self
                .inner
                .streamer
                .read()
                .await
                .as_ref()
                .is_some_and(|streamer| streamer.is_running())
            {
                return;
            }

            attempt = attempt.saturating_add(1);
            self.publish_reconnecting(&lost_device, attempt).await;
            self.publish_state(
                "device_lost",
                Some(lost_device.clone()),
                Some("audio_reconnecting"),
                Some(RETRY_DELAY.as_millis() as u64),
            )
            .await;
            tokio::time::sleep(RETRY_DELAY).await;
            if !self.inner.control.is_current(generation) {
                return;
            }

            let devices = match enumerate_audio_devices() {
                Ok(devices) => devices,
                Err(error) => {
                    debug!("Audio recovery enumeration attempt {attempt} failed: {error}");
                    continue;
                }
            };
            let Some(device) = select_recovery_device(&devices, &config.device) else {
                debug!("No audio device found on recovery attempt {attempt}");
                continue;
            };
            let streamer = Arc::new(AudioStreamer::with_config(AudioStreamerConfig {
                capture: AudioConfig {
                    device_name: device.name.clone(),
                    ..Default::default()
                },
                opus: config.quality.to_opus_config(),
            }));

            if let Err(error) = streamer.start().await {
                debug!(
                    "Audio recovery attempt {attempt} failed with {}: {error}",
                    device.name
                );
                continue;
            }

            // Commit a recovered streamer under the same operation lock used by
            // user-driven start/stop/config updates. Cancellation is rechecked
            // after acquiring the lock so an old task cannot resurrect itself.
            let _operation = self.inner.operation.lock().await;
            if !self.inner.control.is_current(generation) || !self.inner.config.read().await.enabled
            {
                let _ = streamer.stop().await;
                return;
            }

            self.inner.config.write().await.device = device.name.clone();
            *self.inner.streamer.write().await = Some(streamer.clone());
            self.inner.monitor.report_recovered().await;
            self.publish_recovered(&device.name).await;
            if let Some(callback) = self.inner.recovered_callback.read().await.clone() {
                callback();
            }
            self.publish_state("streaming", Some(device.name.clone()), None, None)
                .await;
            info!(
                "Audio recovered with {} after {} attempts",
                device.name, attempt
            );
            self.inner.control.finish(generation);
            self.monitor(streamer, device.name);
            drop(_operation);
            return;
        }
    }

    async fn publish_state(
        &self,
        state: &str,
        device: Option<String>,
        reason: Option<&str>,
        next_retry_ms: Option<u64>,
    ) {
        if let Some(bus) = self.inner.event_bus.read().await.as_ref() {
            bus.publish(SystemEvent::StreamStateChanged {
                kind: StreamKind::Audio,
                state: state.to_string(),
                device,
                reason: reason.map(str::to_string),
                next_retry_ms,
            });
            bus.mark_device_info_dirty();
        }
    }

    async fn publish_device_lost(&self, device: &str, reason: &str) {
        if let Some(bus) = self.inner.event_bus.read().await.as_ref() {
            bus.publish(SystemEvent::StreamDeviceLost {
                kind: StreamKind::Audio,
                device: device.to_string(),
                reason: reason.to_string(),
            });
        }
    }

    async fn publish_reconnecting(&self, device: &str, attempt: u32) {
        if let Some(bus) = self.inner.event_bus.read().await.as_ref() {
            bus.publish(SystemEvent::StreamReconnecting {
                device: device.to_string(),
                attempt,
            });
        }
    }

    async fn publish_recovered(&self, device: &str) {
        if let Some(bus) = self.inner.event_bus.read().await.as_ref() {
            bus.publish(SystemEvent::StreamRecovered {
                device: device.to_string(),
            });
        }
    }
}

pub(super) fn select_recovery_device(
    devices: &[AudioDeviceInfo],
    preferred: &str,
) -> Option<AudioDeviceInfo> {
    devices
        .iter()
        .find(|device| !preferred.trim().is_empty() && device.name == preferred)
        .or_else(|| {
            devices.iter().find(|device| {
                device.is_hdmi
                    && device.sample_rates.contains(&48_000)
                    && device.channels.contains(&2)
            })
        })
        .or_else(|| {
            devices.iter().find(|device| {
                device.sample_rates.contains(&48_000) && device.channels.contains(&2)
            })
        })
        .or_else(|| devices.first())
        .cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn device(name: &str, compatible: bool, hdmi: bool) -> AudioDeviceInfo {
        AudioDeviceInfo {
            name: name.to_string(),
            description: name.to_string(),
            card_index: 0,
            device_index: 0,
            sample_rates: if compatible {
                vec![48_000]
            } else {
                vec![44_100]
            },
            channels: vec![2],
            is_capture: true,
            is_hdmi: hdmi,
            usb_bus: None,
        }
    }

    #[test]
    fn stale_recovery_cannot_finish_a_new_generation() {
        let control = RecoveryControl::new();
        let stale = control.begin().unwrap();
        control.cancel();
        let current = control.begin().unwrap();

        control.finish(stale);
        assert!(control.is_current(current));
    }

    #[test]
    fn completed_recovery_cannot_finish_the_next_recovery() {
        let control = RecoveryControl::new();
        let completed = control.begin().unwrap();
        control.finish(completed);
        let current = control.begin().unwrap();

        control.finish(completed);
        assert!(control.is_current(current));
    }

    #[test]
    fn recovery_prefers_requested_then_compatible_hdmi() {
        let devices = vec![device("fallback", true, false), device("hdmi", true, true)];
        assert_eq!(
            select_recovery_device(&devices, "fallback").unwrap().name,
            "fallback"
        );
        assert_eq!(
            select_recovery_device(&devices, "missing").unwrap().name,
            "hdmi"
        );
    }
}
