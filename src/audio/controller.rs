//! Device selection, quality presets, streaming.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};

use super::capture::AudioConfig;
use super::device::{enumerate_audio_devices_with_current, find_best_audio_device, AudioDeviceInfo};
use super::encoder::OpusFrame;
use super::monitor::AudioHealthMonitor;
use super::streamer::{AudioStreamer, AudioStreamerConfig};
use super::recovery;
use super::types::{AudioControllerConfig, AudioQuality, AudioStatus};
use crate::error::{AppError, Result};
use crate::events::EventBus;

pub(super) type AudioRecoveredCallback = Arc<dyn Fn() + Send + Sync>;

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
        if let Some(bus) = self.event_bus.read().await.as_ref() {
            bus.mark_device_info_dirty();
        }
    }
    fn spawn_recovery_task(&self, lost_device: String, reason: String) {
        recovery::spawn_recovery_task(
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
        recovery::spawn_stream_monitor(
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

        if let Some(streamer) = self.streamer.read().await.as_ref() {
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
        self.streamer
            .read()
            .await
            .as_ref()
            .is_some_and(|streamer| streamer.is_running())
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
