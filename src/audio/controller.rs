//! Device selection, quality presets, streaming.

use serde::{Deserialize, Serialize};
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use super::capture::AudioConfig;
use super::device::{
    enumerate_audio_devices, enumerate_audio_devices_with_current, find_best_audio_device,
    AudioDeviceInfo,
};
use super::encoder::{OpusConfig, OpusFrame};
use super::monitor::AudioHealthMonitor;
use super::streamer::{AudioStreamState, AudioStreamer, AudioStreamerConfig};
use crate::error::{AppError, Result};
use crate::events::{EventBus, SystemEvent};

const AUDIO_RECOVERY_RETRY_DELAY: Duration = Duration::from_secs(1);

type AudioRecoveredCallback = Arc<dyn Fn() + Send + Sync>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum AudioQuality {
    Voice,
    #[default]
    Balanced,
    High,
}

impl AudioQuality {
    pub fn bitrate(&self) -> u32 {
        match self {
            AudioQuality::Voice => 32000,
            AudioQuality::Balanced => 64000,
            AudioQuality::High => 128000,
        }
    }

    pub fn to_opus_config(&self) -> OpusConfig {
        match self {
            AudioQuality::Voice => OpusConfig::voice(),
            AudioQuality::Balanced => OpusConfig::default(),
            AudioQuality::High => OpusConfig::music(),
        }
    }
}

impl FromStr for AudioQuality {
    type Err = AppError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "voice" => Ok(Self::Voice),
            "balanced" => Ok(Self::Balanced),
            "high" => Ok(Self::High),
            _ => Err(AppError::BadRequest(format!(
                "invalid audio quality {:?} (expected voice, balanced, or high)",
                s.trim()
            ))),
        }
    }
}

impl std::fmt::Display for AudioQuality {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AudioQuality::Voice => write!(f, "voice"),
            AudioQuality::Balanced => write!(f, "balanced"),
            AudioQuality::High => write!(f, "high"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct AudioControllerConfig {
    pub enabled: bool,
    pub device: String,
    pub quality: AudioQuality,
}

impl Default for AudioControllerConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            device: String::new(),
            quality: AudioQuality::Balanced,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct AudioStatus {
    pub enabled: bool,
    pub streaming: bool,
    pub device: Option<String>,
    pub quality: AudioQuality,
    pub subscriber_count: usize,
    pub error: Option<String>,
}

pub struct AudioController {
    config: Arc<RwLock<AudioControllerConfig>>,
    streamer: Arc<RwLock<Option<Arc<AudioStreamer>>>>,
    devices: Arc<RwLock<Vec<AudioDeviceInfo>>>,
    event_bus: Arc<RwLock<Option<Arc<EventBus>>>>,
    monitor: Arc<AudioHealthMonitor>,
    recovery_in_progress: Arc<AtomicBool>,
    recovered_callback: Arc<RwLock<Option<AudioRecoveredCallback>>>,
}

impl AudioController {
    pub fn new(config: AudioControllerConfig) -> Self {
        Self {
            config: Arc::new(RwLock::new(config)),
            streamer: Arc::new(RwLock::new(None)),
            devices: Arc::new(RwLock::new(Vec::new())),
            event_bus: Arc::new(RwLock::new(None)),
            monitor: Arc::new(AudioHealthMonitor::new()),
            recovery_in_progress: Arc::new(AtomicBool::new(false)),
            recovered_callback: Arc::new(RwLock::new(None)),
        }
    }

    pub async fn set_event_bus(&self, event_bus: Arc<EventBus>) {
        *self.event_bus.write().await = Some(event_bus);
    }

    pub async fn set_recovered_callback(&self, callback: Arc<dyn Fn() + Send + Sync>) {
        *self.recovered_callback.write().await = Some(callback);
    }

    async fn mark_device_info_dirty(&self) {
        if let Some(ref bus) = *self.event_bus.read().await {
            bus.mark_device_info_dirty();
        }
    }

    async fn publish_state(
        event_bus: &Arc<RwLock<Option<Arc<EventBus>>>>,
        state: &str,
        device: Option<String>,
        reason: Option<&str>,
        next_retry_ms: Option<u64>,
    ) {
        if let Some(ref bus) = *event_bus.read().await {
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
        if let Some(ref bus) = *event_bus.read().await {
            bus.publish(SystemEvent::StreamDeviceLost {
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
        if let Some(ref bus) = *event_bus.read().await {
            bus.publish(SystemEvent::StreamReconnecting {
                device: device.to_string(),
                attempt,
            });
        }
    }

    async fn publish_recovered(event_bus: &Arc<RwLock<Option<Arc<EventBus>>>>, device: &str) {
        if let Some(ref bus) = *event_bus.read().await {
            bus.publish(SystemEvent::StreamRecovered {
                device: device.to_string(),
            });
        }
    }

    fn select_recovery_device(
        devices: &[AudioDeviceInfo],
        preferred: &str,
    ) -> Option<AudioDeviceInfo> {
        if !preferred.trim().is_empty() {
            if let Some(device) = devices.iter().find(|d| d.name == preferred) {
                return Some(device.clone());
            }
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
                Self::spawn_recovery_task_from_parts(
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
            Self::publish_device_lost(&event_bus, &lost_device, &reason).await;
            Self::publish_state(
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

                let cfg = config.read().await.clone();
                if !cfg.enabled {
                    recovery_in_progress.store(false, Ordering::SeqCst);
                    return;
                }

                attempt = attempt.saturating_add(1);
                Self::publish_reconnecting(&event_bus, &lost_device, attempt).await;
                Self::publish_state(
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

                let Some(device) = Self::select_recovery_device(&devices, &cfg.device) else {
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
                        Self::publish_recovered(&event_bus, &device.name).await;
                        if let Some(callback) = recovered_callback.read().await.clone() {
                            callback();
                        }
                        Self::publish_state(
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
                        Self::spawn_stream_monitor_from_parts(
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

    fn spawn_recovery_task(&self, lost_device: String, reason: String) {
        Self::spawn_recovery_task_from_parts(
            self.config.clone(),
            self.streamer.clone(),
            self.event_bus.clone(),
            self.monitor.clone(),
            self.recovery_in_progress.clone(),
            self.recovered_callback.clone(),
            lost_device,
            reason,
        );
    }

    fn spawn_stream_monitor(&self, streamer: Arc<AudioStreamer>, device: String) {
        Self::spawn_stream_monitor_from_parts(
            self.config.clone(),
            self.streamer.clone(),
            self.event_bus.clone(),
            self.monitor.clone(),
            self.recovery_in_progress.clone(),
            self.recovered_callback.clone(),
            streamer,
            device,
        );
    }

    pub async fn list_devices(&self) -> Result<Vec<AudioDeviceInfo>> {
        let current_device = if self.is_streaming().await {
            Some(self.config.read().await.device.clone())
        } else {
            None
        };

        let devices = enumerate_audio_devices_with_current(current_device.as_deref())?;
        *self.devices.write().await = devices.clone();
        Ok(devices)
    }

    pub async fn get_cached_devices(&self) -> Vec<AudioDeviceInfo> {
        self.devices.read().await.clone()
    }

    pub async fn select_device(&self, device: &str) -> Result<()> {
        let devices = self.list_devices().await?;
        let found = devices
            .iter()
            .any(|d| d.name == device || d.description.contains(device));

        if !found {
            return Err(AppError::AudioError(format!(
                "Audio device not found: {}",
                device
            )));
        }

        {
            let mut config = self.config.write().await;
            config.device = device.to_string();
        }

        info!("Audio device selected: {}", device);

        if self.is_streaming().await {
            self.stop_streaming().await?;
            self.start_streaming().await?;
        }

        Ok(())
    }

    pub async fn set_quality(&self, quality: AudioQuality) -> Result<()> {
        {
            let mut config = self.config.write().await;
            config.quality = quality;
        }

        if let Some(ref streamer) = *self.streamer.read().await {
            streamer.set_bitrate(quality.bitrate()).await?;
        }

        info!(
            "Audio quality set to: {:?} ({}bps)",
            quality,
            quality.bitrate()
        );
        Ok(())
    }

    pub async fn start_streaming(&self) -> Result<()> {
        {
            let config = self.config.read().await;
            if !config.enabled {
                return Err(AppError::AudioError("Audio is disabled".to_string()));
            }
        }

        if self.is_streaming().await {
            return Ok(());
        }

        let mut select_error = None;
        let (device_name, quality) = {
            let mut cfg = self.config.write().await;
            if cfg.device.trim().is_empty() {
                match find_best_audio_device() {
                    Ok(best) => cfg.device = best.name,
                    Err(e) => {
                        select_error = Some(format!("Failed to select audio device: {}", e));
                    }
                }
            }
            (cfg.device.clone(), cfg.quality)
        };

        if let Some(error_msg) = select_error {
            self.monitor.report_error(&error_msg, "start_failed").await;
            self.spawn_recovery_task("auto".to_string(), error_msg.clone());
            self.mark_device_info_dirty().await;
            return Err(AppError::AudioError(error_msg));
        }

        debug!("Starting audio streaming with device: {}", device_name);

        self.monitor.prepare_retry_attempt();

        let streamer_config = AudioStreamerConfig {
            capture: AudioConfig {
                device_name: device_name.clone(),
                ..Default::default()
            },
            opus: quality.to_opus_config(),
        };

        let streamer = Arc::new(AudioStreamer::with_config(streamer_config));

        if let Err(e) = streamer.start().await {
            let error_msg = format!("Failed to start audio: {}", e);

            self.monitor.report_error(&error_msg, "start_failed").await;
            self.spawn_recovery_task(device_name.clone(), error_msg.clone());

            self.mark_device_info_dirty().await;

            return Err(AppError::AudioError(error_msg));
        }

        let streamer_for_monitor = streamer.clone();
        *self.streamer.write().await = Some(streamer);
        self.spawn_stream_monitor(streamer_for_monitor, device_name.clone());

        if self.monitor.is_error().await {
            self.monitor.report_recovered().await;
        }

        self.recovery_in_progress.store(false, Ordering::SeqCst);

        self.mark_device_info_dirty().await;

        info!("Audio streaming started");
        Ok(())
    }

    pub async fn stop_streaming(&self) -> Result<()> {
        self.recovery_in_progress.store(false, Ordering::SeqCst);

        if let Some(streamer) = self.streamer.write().await.take() {
            streamer.stop().await?;
        }

        self.monitor.reset().await;
        self.mark_device_info_dirty().await;

        info!("Audio streaming stopped");
        Ok(())
    }

    pub async fn is_streaming(&self) -> bool {
        if let Some(ref streamer) = *self.streamer.read().await {
            streamer.is_running()
        } else {
            false
        }
    }

    pub async fn status(&self) -> AudioStatus {
        let (enabled, device_str, quality) = {
            let c = self.config.read().await;
            (c.enabled, c.device.clone(), c.quality)
        };
        let error = self.monitor.error_message().await;

        let (streaming, subscriber_count) = if let Some(ref streamer) = *self.streamer.read().await
        {
            let streaming = streamer.is_running();
            let subscriber_count = streamer.stats().subscriber_count;
            (streaming, subscriber_count)
        } else {
            (false, 0)
        };

        AudioStatus {
            enabled,
            streaming,
            device: if streaming || enabled {
                Some(device_str)
            } else {
                None
            },
            quality,
            subscriber_count,
            error,
        }
    }

    pub async fn subscribe_opus(&self) -> Option<tokio::sync::mpsc::Receiver<Arc<OpusFrame>>> {
        self.streamer
            .read()
            .await
            .as_ref()
            .map(|s| s.subscribe_opus())
    }

    pub async fn set_enabled(&self, enabled: bool) -> Result<()> {
        {
            let mut config = self.config.write().await;
            config.enabled = enabled;
        }

        if !enabled && self.is_streaming().await {
            self.stop_streaming().await?;
        }

        info!("Audio enabled: {}", enabled);
        Ok(())
    }

    pub async fn update_config(&self, new_config: AudioControllerConfig) -> Result<()> {
        let was_streaming = self.is_streaming().await;

        if was_streaming {
            self.stop_streaming().await?;
        }

        *self.config.write().await = new_config.clone();

        if new_config.enabled {
            self.start_streaming().await?;
        }

        Ok(())
    }

    pub async fn shutdown(&self) -> Result<()> {
        self.stop_streaming().await
    }
}

impl Default for AudioController {
    fn default() -> Self {
        Self::new(AudioControllerConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_quality_bitrate() {
        assert_eq!(AudioQuality::Voice.bitrate(), 32000);
        assert_eq!(AudioQuality::Balanced.bitrate(), 64000);
        assert_eq!(AudioQuality::High.bitrate(), 128000);
    }

    #[test]
    fn test_audio_quality_from_str() {
        assert_eq!(
            "voice".parse::<AudioQuality>().unwrap(),
            AudioQuality::Voice
        );
        assert_eq!(
            "balanced".parse::<AudioQuality>().unwrap(),
            AudioQuality::Balanced
        );
        assert_eq!("high".parse::<AudioQuality>().unwrap(), AudioQuality::High);
    }

    #[test]
    fn test_audio_quality_from_str_rejects_aliases_and_unknown() {
        assert!("low".parse::<AudioQuality>().is_err());
        assert!("music".parse::<AudioQuality>().is_err());
        assert!("unknown".parse::<AudioQuality>().is_err());
        assert!("".parse::<AudioQuality>().is_err());
    }

    #[tokio::test]
    async fn test_controller_default() {
        let controller = AudioController::default();
        let status = controller.status().await;
        assert!(!status.enabled);
        assert!(!status.streaming);
    }
}
