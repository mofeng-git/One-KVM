//! Device selection, quality presets, streaming.

use serde::{Deserialize, Serialize};
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

use super::capture::AudioConfig;
use super::device::{
    enumerate_audio_devices_with_current, find_best_audio_device, AudioDeviceInfo,
};
use super::encoder::{OpusConfig, OpusFrame};
use super::monitor::AudioHealthMonitor;
use super::streamer::{AudioStreamer, AudioStreamerConfig};
use crate::error::{AppError, Result};
use crate::events::EventBus;

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
    config: RwLock<AudioControllerConfig>,
    streamer: RwLock<Option<Arc<AudioStreamer>>>,
    devices: RwLock<Vec<AudioDeviceInfo>>,
    event_bus: RwLock<Option<Arc<EventBus>>>,
    monitor: Arc<AudioHealthMonitor>,
}

impl AudioController {
    pub fn new(config: AudioControllerConfig) -> Self {
        Self {
            config: RwLock::new(config),
            streamer: RwLock::new(None),
            devices: RwLock::new(Vec::new()),
            event_bus: RwLock::new(None),
            monitor: Arc::new(AudioHealthMonitor::new()),
        }
    }

    pub async fn set_event_bus(&self, event_bus: Arc<EventBus>) {
        *self.event_bus.write().await = Some(event_bus);
    }

    async fn mark_device_info_dirty(&self) {
        if let Some(ref bus) = *self.event_bus.read().await {
            bus.mark_device_info_dirty();
        }
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

        let (device_name, quality) = {
            let mut cfg = self.config.write().await;
            if cfg.device.trim().is_empty() {
                let best = find_best_audio_device()?;
                cfg.device = best.name;
            }
            (cfg.device.clone(), cfg.quality)
        };

        info!("Starting audio streaming with device: {}", device_name);

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

            self.mark_device_info_dirty().await;

            return Err(AppError::AudioError(error_msg));
        }

        *self.streamer.write().await = Some(streamer);

        if self.monitor.is_error().await {
            self.monitor.report_recovered().await;
        }

        self.mark_device_info_dirty().await;

        info!("Audio streaming started");
        Ok(())
    }

    pub async fn stop_streaming(&self) -> Result<()> {
        if let Some(streamer) = self.streamer.write().await.take() {
            streamer.stop().await?;
        }

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
