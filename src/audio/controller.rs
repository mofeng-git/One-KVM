//! Audio controller for high-level audio management
//!
//! Provides device enumeration, selection, quality control, and streaming management.

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tracing::info;

use super::capture::AudioConfig;
use super::device::{enumerate_audio_devices_with_current, AudioDeviceInfo};
use super::encoder::{OpusConfig, OpusFrame};
use super::monitor::{AudioHealthMonitor, AudioHealthStatus};
use super::streamer::{AudioStreamer, AudioStreamerConfig};
use crate::error::{AppError, Result};
use crate::events::{EventBus, SystemEvent};

/// Audio quality presets
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum AudioQuality {
    /// Low bandwidth voice (32kbps)
    Voice,
    /// Balanced quality (64kbps) - default
    #[default]
    Balanced,
    /// High quality audio (128kbps)
    High,
}

impl AudioQuality {
    /// Get the bitrate for this quality level
    pub fn bitrate(&self) -> u32 {
        match self {
            AudioQuality::Voice => 32000,
            AudioQuality::Balanced => 64000,
            AudioQuality::High => 128000,
        }
    }

    /// Parse from string
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "voice" | "low" => AudioQuality::Voice,
            "high" | "music" => AudioQuality::High,
            _ => AudioQuality::Balanced,
        }
    }

    /// Convert to OpusConfig
    pub fn to_opus_config(&self) -> OpusConfig {
        match self {
            AudioQuality::Voice => OpusConfig::voice(),
            AudioQuality::Balanced => OpusConfig::default(),
            AudioQuality::High => OpusConfig::music(),
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

/// Audio controller configuration
///
/// Note: Sample rate is fixed at 48000Hz and channels at 2 (stereo).
/// These are optimal for Opus encoding and match WebRTC requirements.
#[derive(Debug, Clone)]
pub struct AudioControllerConfig {
    /// Whether audio is enabled
    pub enabled: bool,
    /// Selected device name
    pub device: String,
    /// Audio quality preset
    pub quality: AudioQuality,
}

impl Default for AudioControllerConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            device: "default".to_string(),
            quality: AudioQuality::Balanced,
        }
    }
}

/// Current audio status
#[derive(Debug, Clone, Serialize)]
pub struct AudioStatus {
    /// Whether audio feature is enabled
    pub enabled: bool,
    /// Whether audio is currently streaming
    pub streaming: bool,
    /// Currently selected device
    pub device: Option<String>,
    /// Current quality preset
    pub quality: AudioQuality,
    /// Number of connected subscribers
    pub subscriber_count: usize,
    /// Frames encoded
    pub frames_encoded: u64,
    /// Bytes output
    pub bytes_output: u64,
    /// Error message if any
    pub error: Option<String>,
}

/// Audio controller
///
/// High-level interface for audio management, providing:
/// - Device enumeration and selection
/// - Quality control
/// - Stream start/stop
/// - Status reporting
pub struct AudioController {
    config: RwLock<AudioControllerConfig>,
    streamer: RwLock<Option<Arc<AudioStreamer>>>,
    devices: RwLock<Vec<AudioDeviceInfo>>,
    event_bus: RwLock<Option<Arc<EventBus>>>,
    last_error: RwLock<Option<String>>,
    /// Health monitor for error tracking and recovery
    monitor: Arc<AudioHealthMonitor>,
}

impl AudioController {
    /// Create a new audio controller with configuration
    pub fn new(config: AudioControllerConfig) -> Self {
        Self {
            config: RwLock::new(config),
            streamer: RwLock::new(None),
            devices: RwLock::new(Vec::new()),
            event_bus: RwLock::new(None),
            last_error: RwLock::new(None),
            monitor: Arc::new(AudioHealthMonitor::with_defaults()),
        }
    }

    /// Set event bus for publishing audio events
    pub async fn set_event_bus(&self, event_bus: Arc<EventBus>) {
        *self.event_bus.write().await = Some(event_bus.clone());
        // Also set event bus on the monitor for health notifications
        self.monitor.set_event_bus(event_bus).await;
    }

    /// Publish an event to the event bus
    async fn publish_event(&self, event: SystemEvent) {
        if let Some(ref bus) = *self.event_bus.read().await {
            bus.publish(event);
        }
    }

    /// List available audio capture devices
    pub async fn list_devices(&self) -> Result<Vec<AudioDeviceInfo>> {
        // Get current device if streaming (it may be busy and unable to be opened)
        let current_device = if self.is_streaming().await {
            Some(self.config.read().await.device.clone())
        } else {
            None
        };

        let devices = enumerate_audio_devices_with_current(current_device.as_deref())?;
        *self.devices.write().await = devices.clone();
        Ok(devices)
    }

    /// Refresh device list and cache it
    pub async fn refresh_devices(&self) -> Result<()> {
        // Get current device if streaming (it may be busy and unable to be opened)
        let current_device = if self.is_streaming().await {
            Some(self.config.read().await.device.clone())
        } else {
            None
        };

        let devices = enumerate_audio_devices_with_current(current_device.as_deref())?;
        *self.devices.write().await = devices;
        Ok(())
    }

    /// Get cached device list
    pub async fn get_cached_devices(&self) -> Vec<AudioDeviceInfo> {
        self.devices.read().await.clone()
    }

    /// Select audio device
    pub async fn select_device(&self, device: &str) -> Result<()> {
        // Validate device exists
        let devices = self.list_devices().await?;
        let found = devices.iter().any(|d| d.name == device || d.description.contains(device));

        if !found && device != "default" {
            return Err(AppError::AudioError(format!(
                "Audio device not found: {}",
                device
            )));
        }

        // Update config
        {
            let mut config = self.config.write().await;
            config.device = device.to_string();
        }

        // Publish event
        self.publish_event(SystemEvent::AudioDeviceSelected {
            device: device.to_string(),
        })
        .await;

        info!("Audio device selected: {}", device);

        // If streaming, restart with new device
        if self.is_streaming().await {
            self.stop_streaming().await?;
            self.start_streaming().await?;
        }

        Ok(())
    }

    /// Set audio quality
    pub async fn set_quality(&self, quality: AudioQuality) -> Result<()> {
        // Update config
        {
            let mut config = self.config.write().await;
            config.quality = quality;
        }

        // Update streamer if running
        if let Some(ref streamer) = *self.streamer.read().await {
            streamer.set_bitrate(quality.bitrate()).await?;
        }

        // Publish event
        self.publish_event(SystemEvent::AudioQualityChanged {
            quality: quality.to_string(),
        })
        .await;

        info!("Audio quality set to: {:?} ({}bps)", quality, quality.bitrate());
        Ok(())
    }

    /// Start audio streaming
    pub async fn start_streaming(&self) -> Result<()> {
        let config = self.config.read().await.clone();

        if !config.enabled {
            return Err(AppError::AudioError("Audio is disabled".to_string()));
        }

        // Check if already streaming
        if self.is_streaming().await {
            return Ok(());
        }

        info!("Starting audio streaming with device: {}", config.device);

        // Clear any previous error
        *self.last_error.write().await = None;

        // Create streamer config (fixed 48kHz stereo)
        let streamer_config = AudioStreamerConfig {
            capture: AudioConfig {
                device_name: config.device.clone(),
                ..Default::default()
            },
            opus: config.quality.to_opus_config(),
        };

        // Create and start streamer
        let streamer = Arc::new(AudioStreamer::with_config(streamer_config));

        if let Err(e) = streamer.start().await {
            let error_msg = format!("Failed to start audio: {}", e);
            *self.last_error.write().await = Some(error_msg.clone());

            // Report error to health monitor
            self.monitor
                .report_error(Some(&config.device), &error_msg, "start_failed")
                .await;

            self.publish_event(SystemEvent::AudioStateChanged {
                streaming: false,
                device: None,
            })
            .await;

            return Err(AppError::AudioError(error_msg));
        }

        *self.streamer.write().await = Some(streamer);

        // Report recovery if we were in an error state
        if self.monitor.is_error().await {
            self.monitor.report_recovered(Some(&config.device)).await;
        }

        // Publish event
        self.publish_event(SystemEvent::AudioStateChanged {
            streaming: true,
            device: Some(config.device),
        })
        .await;

        info!("Audio streaming started");
        Ok(())
    }

    /// Stop audio streaming
    pub async fn stop_streaming(&self) -> Result<()> {
        if let Some(streamer) = self.streamer.write().await.take() {
            streamer.stop().await?;
        }

        // Publish event
        self.publish_event(SystemEvent::AudioStateChanged {
            streaming: false,
            device: None,
        })
        .await;

        info!("Audio streaming stopped");
        Ok(())
    }

    /// Check if currently streaming
    pub async fn is_streaming(&self) -> bool {
        if let Some(ref streamer) = *self.streamer.read().await {
            streamer.is_running()
        } else {
            false
        }
    }

    /// Get current status
    pub async fn status(&self) -> AudioStatus {
        let config = self.config.read().await;
        let streaming = self.is_streaming().await;
        let error = self.last_error.read().await.clone();

        let (subscriber_count, frames_encoded, bytes_output) = if let Some(ref streamer) =
            *self.streamer.read().await
        {
            let stats = streamer.stats().await;
            (stats.subscriber_count, stats.frames_encoded, stats.bytes_output)
        } else {
            (0, 0, 0)
        };

        AudioStatus {
            enabled: config.enabled,
            streaming,
            device: if streaming || config.enabled {
                Some(config.device.clone())
            } else {
                None
            },
            quality: config.quality,
            subscriber_count,
            frames_encoded,
            bytes_output,
            error,
        }
    }

    /// Subscribe to Opus frames (for WebSocket clients)
    pub fn subscribe_opus(&self) -> Option<broadcast::Receiver<OpusFrame>> {
        // Use try_read to avoid blocking - this is called from sync context sometimes
        if let Ok(guard) = self.streamer.try_read() {
            guard.as_ref().map(|s| s.subscribe_opus())
        } else {
            None
        }
    }

    /// Subscribe to Opus frames (async version)
    pub async fn subscribe_opus_async(&self) -> Option<broadcast::Receiver<OpusFrame>> {
        self.streamer.read().await.as_ref().map(|s| s.subscribe_opus())
    }

    /// Enable or disable audio
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

    /// Update full configuration
    pub async fn update_config(&self, new_config: AudioControllerConfig) -> Result<()> {
        let was_streaming = self.is_streaming().await;
        let old_config = self.config.read().await.clone();

        // Stop streaming if running
        if was_streaming {
            self.stop_streaming().await?;
        }

        // Update config
        *self.config.write().await = new_config.clone();

        // Restart streaming if it was running and still enabled
        if was_streaming && new_config.enabled {
            self.start_streaming().await?;
        }

        // Publish events for changes
        if old_config.device != new_config.device {
            self.publish_event(SystemEvent::AudioDeviceSelected {
                device: new_config.device.clone(),
            })
            .await;
        }

        if old_config.quality != new_config.quality {
            self.publish_event(SystemEvent::AudioQualityChanged {
                quality: new_config.quality.to_string(),
            })
            .await;
        }

        Ok(())
    }

    /// Shutdown the controller
    pub async fn shutdown(&self) -> Result<()> {
        self.stop_streaming().await
    }

    /// Get the health monitor reference
    pub fn monitor(&self) -> &Arc<AudioHealthMonitor> {
        &self.monitor
    }

    /// Get current health status
    pub async fn health_status(&self) -> AudioHealthStatus {
        self.monitor.status().await
    }

    /// Check if the audio is healthy
    pub async fn is_healthy(&self) -> bool {
        self.monitor.is_healthy().await
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
        assert_eq!(AudioQuality::from_str("voice"), AudioQuality::Voice);
        assert_eq!(AudioQuality::from_str("low"), AudioQuality::Voice);
        assert_eq!(AudioQuality::from_str("balanced"), AudioQuality::Balanced);
        assert_eq!(AudioQuality::from_str("high"), AudioQuality::High);
        assert_eq!(AudioQuality::from_str("music"), AudioQuality::High);
        assert_eq!(AudioQuality::from_str("unknown"), AudioQuality::Balanced);
    }

    #[tokio::test]
    async fn test_controller_default() {
        let controller = AudioController::default();
        let status = controller.status().await;
        assert!(!status.enabled);
        assert!(!status.streaming);
    }
}
