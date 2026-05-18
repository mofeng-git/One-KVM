use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use super::capture::AudioConfig;
use super::device::{enumerate_audio_devices, AudioDeviceInfo};
use super::monitor::AudioHealthMonitor;
use super::streamer::{AudioStreamState, AudioStreamer, AudioStreamerConfig};
use super::types::AudioControllerConfig;
use super::controller::AudioRecoveredCallback;
use crate::events::{EventBus, StreamDeviceLostKind, SystemEvent};

const AUDIO_RECOVERY_RETRY_DELAY: std::time::Duration = std::time::Duration::from_secs(1);

pub(super) fn select_recovery_device(
    devices: &[AudioDeviceInfo],
    preferred: &str,
) -> Option<AudioDeviceInfo> {
    if let Some(device) = devices
        .iter()
        .find(|d| !preferred.trim().is_empty() && d.name == preferred)
    {
        return Some(device.clone());
    }

    devices
        .iter()
        .find(|d| d.is_hdmi && d.sample_rates.contains(&48_000) && d.channels.contains(&2))
        .or_else(|| {
            devices
                .iter()
                .find(|d| d.sample_rates.contains(&48_000) && d.channels.contains(&2))
        })
        .or_else(|| devices.first())
        .cloned()
}

async fn publish_state(
    event_bus: &Arc<RwLock<Option<Arc<EventBus>>>>,
    state: &str,
    device: Option<String>,
    reason: Option<&str>,
    next_retry_ms: Option<u64>,
) {
    if let Some(bus) = event_bus.read().await.as_ref() {
        bus.publish(SystemEvent::StreamStateChanged {
            state: state.to_string(),
            device,
            reason: reason.map(str::to_string),
            next_retry_ms,
        });
        bus.mark_device_info_dirty();
    }
}

async fn publish_device_lost(
    event_bus: &Arc<RwLock<Option<Arc<EventBus>>>>,
    device: &str,
    reason: &str,
) {
    if let Some(bus) = event_bus.read().await.as_ref() {
        bus.publish(SystemEvent::StreamDeviceLost {
            kind: StreamDeviceLostKind::Audio,
            device: device.to_string(),
            reason: reason.to_string(),
        });
    }
}

async fn publish_reconnecting(
    event_bus: &Arc<RwLock<Option<Arc<EventBus>>>>,
    device: &str,
    attempt: u32,
) {
    if let Some(bus) = event_bus.read().await.as_ref() {
        bus.publish(SystemEvent::StreamReconnecting {
            device: device.to_string(),
            attempt,
        });
    }
}

async fn publish_recovered(event_bus: &Arc<RwLock<Option<Arc<EventBus>>>>, device: &str) {
    if let Some(bus) = event_bus.read().await.as_ref() {
        bus.publish(SystemEvent::StreamRecovered {
            device: device.to_string(),
        });
    }
}

fn spawn_stream_monitor_from_parts(
    config: Arc<RwLock<AudioControllerConfig>>,
    streamer_slot: Arc<RwLock<Option<Arc<AudioStreamer>>>>,
    event_bus: Arc<RwLock<Option<Arc<EventBus>>>>,
    monitor: Arc<AudioHealthMonitor>,
    recovery_in_progress: Arc<AtomicBool>,
    recovered_callback: Arc<RwLock<Option<AudioRecoveredCallback>>>,
    streamer: Arc<AudioStreamer>,
    device: String,
) {
    let mut state_rx = streamer.state_watch();

    tokio::spawn(async move {
        loop {
            if state_rx.changed().await.is_err() {
                return;
            }

            if *state_rx.borrow() != AudioStreamState::Error {
                continue;
            }

            {
                let current = streamer_slot.read().await;
                if !current
                    .as_ref()
                    .is_some_and(|current| Arc::ptr_eq(current, &streamer))
                {
                    return;
                }
            }

            let reason = format!("Audio device lost: {}", device);
            monitor.report_error(&reason, "device_lost").await;
            spawn_recovery_task_from_parts(
                config,
                streamer_slot,
                event_bus,
                monitor,
                recovery_in_progress,
                recovered_callback,
                device,
                reason,
            );
            return;
        }
    });
}

fn spawn_recovery_task_from_parts(
    config: Arc<RwLock<AudioControllerConfig>>,
    streamer_slot: Arc<RwLock<Option<Arc<AudioStreamer>>>>,
    event_bus: Arc<RwLock<Option<Arc<EventBus>>>>,
    monitor: Arc<AudioHealthMonitor>,
    recovery_in_progress: Arc<AtomicBool>,
    recovered_callback: Arc<RwLock<Option<AudioRecoveredCallback>>>,
    lost_device: String,
    reason: String,
) {
    if recovery_in_progress.swap(true, Ordering::SeqCst) {
        debug!("Audio recovery already in progress");
        return;
    }

    tokio::spawn(async move {
        warn!("Audio recovery started for {}: {}", lost_device, reason);
        publish_device_lost(&event_bus, &lost_device, &reason).await;
        publish_state(
            &event_bus,
            "device_lost",
            Some(lost_device.clone()),
            Some("audio_device_lost"),
            Some(AUDIO_RECOVERY_RETRY_DELAY.as_millis() as u64),
        )
        .await;

        let mut attempt = 0u32;

        loop {
            if !recovery_in_progress.load(Ordering::SeqCst) {
                debug!("Audio recovery canceled");
                return;
            }

            if streamer_slot
                .read()
                .await
                .as_ref()
                .is_some_and(|s| s.is_running())
            {
                recovery_in_progress.store(false, Ordering::SeqCst);
                return;
            }

            let cfg: AudioControllerConfig = config.read().await.clone();
            if !cfg.enabled {
                recovery_in_progress.store(false, Ordering::SeqCst);
                return;
            }

            attempt = attempt.saturating_add(1);
            publish_reconnecting(&event_bus, &lost_device, attempt).await;
            publish_state(
                &event_bus,
                "device_lost",
                Some(lost_device.clone()),
                Some("audio_reconnecting"),
                Some(AUDIO_RECOVERY_RETRY_DELAY.as_millis() as u64),
            )
            .await;

            tokio::time::sleep(AUDIO_RECOVERY_RETRY_DELAY).await;

            let devices = match enumerate_audio_devices() {
                Ok(devices) => devices,
                Err(e) => {
                    debug!(
                        "Audio recovery enumerate failed (attempt {}): {}",
                        attempt, e
                    );
                    continue;
                }
            };

            let Some(device) = select_recovery_device(&devices, &cfg.device) else {
                debug!("No audio devices found during recovery attempt {}", attempt);
                continue;
            };

            let streamer_config = AudioStreamerConfig {
                capture: AudioConfig {
                    device_name: device.name.clone(),
                    ..Default::default()
                },
                opus: cfg.quality.to_opus_config(),
            };
            let new_streamer = Arc::new(AudioStreamer::with_config(streamer_config));

            match new_streamer.start().await {
                Ok(()) => {
                    {
                        let mut cfg = config.write().await;
                        cfg.device = device.name.clone();
                    }
                    *streamer_slot.write().await = Some(new_streamer.clone());
                    monitor.report_recovered().await;
                    publish_recovered(&event_bus, &device.name).await;
                    if let Some(callback) = recovered_callback.read().await.clone() {
                        callback();
                    }
                    publish_state(
                        &event_bus,
                        "streaming",
                        Some(device.name.clone()),
                        None,
                        None,
                    )
                    .await;
                    recovery_in_progress.store(false, Ordering::SeqCst);
                    info!(
                        "Audio device recovered with {} after {} attempts",
                        device.name, attempt
                    );
                    spawn_stream_monitor_from_parts(
                        config,
                        streamer_slot,
                        event_bus,
                        monitor,
                        recovery_in_progress,
                        recovered_callback,
                        new_streamer,
                        device.name,
                    );
                    return;
                }
                Err(e) => {
                    debug!(
                        "Audio recovery start failed with {} (attempt {}): {}",
                        device.name, attempt, e
                    );
                }
            }
        }
    });
}

pub(super) fn spawn_stream_monitor(
    config: Arc<RwLock<AudioControllerConfig>>,
    streamer_slot: Arc<RwLock<Option<Arc<AudioStreamer>>>>,
    event_bus: Arc<RwLock<Option<Arc<EventBus>>>>,
    monitor: Arc<AudioHealthMonitor>,
    recovery_in_progress: Arc<AtomicBool>,
    recovered_callback: Arc<RwLock<Option<AudioRecoveredCallback>>>,
    streamer: Arc<AudioStreamer>,
    device: String,
) {
    spawn_stream_monitor_from_parts(
        config,
        streamer_slot,
        event_bus,
        monitor,
        recovery_in_progress,
        recovered_callback,
        streamer,
        device,
    );
}

pub(super) fn spawn_recovery_task(
    config: Arc<RwLock<AudioControllerConfig>>,
    streamer_slot: Arc<RwLock<Option<Arc<AudioStreamer>>>>,
    event_bus: Arc<RwLock<Option<Arc<EventBus>>>>,
    monitor: Arc<AudioHealthMonitor>,
    recovery_in_progress: Arc<AtomicBool>,
    recovered_callback: Arc<RwLock<Option<AudioRecoveredCallback>>>,
    lost_device: String,
    reason: String,
) {
    spawn_recovery_task_from_parts(
        config,
        streamer_slot,
        event_bus,
        monitor,
        recovery_in_progress,
        recovered_callback,
        lost_device,
        reason,
    );
}
