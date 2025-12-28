//! MJPEG Streamer - High-level MJPEG/HTTP streaming manager
//!
//! This module provides a unified interface for MJPEG streaming mode,
//! integrating video capture, MJPEG distribution, and WebSocket HID.
//!
//! # Architecture
//!
//! ```text
//! MjpegStreamer
//!     |
//!     +-- VideoCapturer (V4L2 video capture)
//!     +-- MjpegStreamHandler (HTTP multipart video)
//!     +-- WsHidHandler (WebSocket HID)
//! ```
//!
//! Note: Audio WebSocket is handled separately by audio_ws.rs (/api/ws/audio)

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tracing::info;

use crate::audio::AudioController;
use crate::error::{AppError, Result};
use crate::events::{EventBus, SystemEvent};
use crate::hid::HidController;
use crate::video::capture::{CaptureConfig, VideoCapturer};
use crate::video::device::{enumerate_devices, find_best_device, VideoDeviceInfo};
use crate::video::format::{PixelFormat, Resolution};
use crate::video::frame::VideoFrame;

use super::mjpeg::MjpegStreamHandler;
use super::ws_hid::WsHidHandler;

/// MJPEG streamer configuration
#[derive(Debug, Clone)]
pub struct MjpegStreamerConfig {
    /// Device path (None = auto-detect)
    pub device_path: Option<PathBuf>,
    /// Desired resolution
    pub resolution: Resolution,
    /// Desired format
    pub format: PixelFormat,
    /// Desired FPS
    pub fps: u32,
    /// JPEG quality (1-100)
    pub jpeg_quality: u8,
}

impl Default for MjpegStreamerConfig {
    fn default() -> Self {
        Self {
            device_path: None,
            resolution: Resolution::HD1080,
            format: PixelFormat::Mjpeg,
            fps: 30,
            jpeg_quality: 80,
        }
    }
}

/// MJPEG streamer state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MjpegStreamerState {
    /// Not initialized
    Uninitialized,
    /// Ready but not streaming
    Ready,
    /// Actively streaming
    Streaming,
    /// No video signal
    NoSignal,
    /// Error occurred
    Error,
}

impl std::fmt::Display for MjpegStreamerState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MjpegStreamerState::Uninitialized => write!(f, "uninitialized"),
            MjpegStreamerState::Ready => write!(f, "ready"),
            MjpegStreamerState::Streaming => write!(f, "streaming"),
            MjpegStreamerState::NoSignal => write!(f, "no_signal"),
            MjpegStreamerState::Error => write!(f, "error"),
        }
    }
}

/// MJPEG streamer statistics
#[derive(Debug, Clone, Default)]
pub struct MjpegStreamerStats {
    /// Current state
    pub state: String,
    /// Current device path
    pub device: Option<String>,
    /// Video resolution
    pub resolution: Option<(u32, u32)>,
    /// Video format
    pub format: Option<String>,
    /// Current FPS
    pub fps: u32,
    /// MJPEG client count
    pub mjpeg_clients: u64,
    /// WebSocket HID client count
    pub ws_hid_clients: usize,
    /// Total frames captured
    pub frames_captured: u64,
}

/// MJPEG Streamer
///
/// High-level manager for MJPEG/HTTP streaming mode.
/// Integrates video capture, MJPEG distribution, and WebSocket HID.
pub struct MjpegStreamer {
    // === Video ===
    config: RwLock<MjpegStreamerConfig>,
    capturer: RwLock<Option<Arc<VideoCapturer>>>,
    mjpeg_handler: Arc<MjpegStreamHandler>,
    current_device: RwLock<Option<VideoDeviceInfo>>,
    state: RwLock<MjpegStreamerState>,

    // === Audio (controller reference only, WS handled by audio_ws.rs) ===
    audio_controller: RwLock<Option<Arc<AudioController>>>,
    audio_enabled: AtomicBool,

    // === HID ===
    ws_hid_handler: Arc<WsHidHandler>,
    hid_controller: RwLock<Option<Arc<HidController>>>,

    // === Control ===
    start_lock: tokio::sync::Mutex<()>,
    events: RwLock<Option<Arc<EventBus>>>,
    config_changing: AtomicBool,
}

impl MjpegStreamer {
    /// Create a new MJPEG streamer
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            config: RwLock::new(MjpegStreamerConfig::default()),
            capturer: RwLock::new(None),
            mjpeg_handler: Arc::new(MjpegStreamHandler::new()),
            current_device: RwLock::new(None),
            state: RwLock::new(MjpegStreamerState::Uninitialized),
            audio_controller: RwLock::new(None),
            audio_enabled: AtomicBool::new(false),
            ws_hid_handler: WsHidHandler::new(),
            hid_controller: RwLock::new(None),
            start_lock: tokio::sync::Mutex::new(()),
            events: RwLock::new(None),
            config_changing: AtomicBool::new(false),
        })
    }

    /// Create with specific config
    pub fn with_config(config: MjpegStreamerConfig) -> Arc<Self> {
        Arc::new(Self {
            config: RwLock::new(config),
            capturer: RwLock::new(None),
            mjpeg_handler: Arc::new(MjpegStreamHandler::new()),
            current_device: RwLock::new(None),
            state: RwLock::new(MjpegStreamerState::Uninitialized),
            audio_controller: RwLock::new(None),
            audio_enabled: AtomicBool::new(false),
            ws_hid_handler: WsHidHandler::new(),
            hid_controller: RwLock::new(None),
            start_lock: tokio::sync::Mutex::new(()),
            events: RwLock::new(None),
            config_changing: AtomicBool::new(false),
        })
    }

    // ========================================================================
    // Configuration and Setup
    // ========================================================================

    /// Set event bus for broadcasting state changes
    pub async fn set_event_bus(&self, events: Arc<EventBus>) {
        *self.events.write().await = Some(events);
    }

    /// Set audio controller (for reference, WebSocket handled by audio_ws.rs)
    pub async fn set_audio_controller(&self, audio: Arc<AudioController>) {
        *self.audio_controller.write().await = Some(audio);
        info!("MjpegStreamer: Audio controller set");
    }

    /// Set HID controller
    pub async fn set_hid_controller(&self, hid: Arc<HidController>) {
        *self.hid_controller.write().await = Some(hid.clone());
        self.ws_hid_handler.set_hid_controller(hid);
        info!("MjpegStreamer: HID controller set");
    }

    /// Enable or disable audio
    pub fn set_audio_enabled(&self, enabled: bool) {
        self.audio_enabled.store(enabled, Ordering::SeqCst);
    }

    /// Check if audio is enabled
    pub fn is_audio_enabled(&self) -> bool {
        self.audio_enabled.load(Ordering::SeqCst)
    }

    // ========================================================================
    // State and Status
    // ========================================================================

    /// Get current state
    pub async fn state(&self) -> MjpegStreamerState {
        *self.state.read().await
    }

    /// Check if config is currently being changed
    pub fn is_config_changing(&self) -> bool {
        self.config_changing.load(Ordering::SeqCst)
    }

    /// Get current device info
    pub async fn current_device(&self) -> Option<VideoDeviceInfo> {
        self.current_device.read().await.clone()
    }

    /// Get statistics
    pub async fn stats(&self) -> MjpegStreamerStats {
        let state = *self.state.read().await;
        let device = self.current_device.read().await;
        let config = self.config.read().await;

        let (resolution, format, frames_captured) = if let Some(ref cap) = *self.capturer.read().await {
            let stats = cap.stats().await;
            (
                Some((config.resolution.width, config.resolution.height)),
                Some(config.format.to_string()),
                stats.frames_captured,
            )
        } else {
            (None, None, 0)
        };

        MjpegStreamerStats {
            state: state.to_string(),
            device: device.as_ref().map(|d| d.path.display().to_string()),
            resolution,
            format,
            fps: config.fps,
            mjpeg_clients: self.mjpeg_handler.client_count(),
            ws_hid_clients: self.ws_hid_handler.client_count(),
            frames_captured,
        }
    }

    // ========================================================================
    // Handler Access
    // ========================================================================

    /// Get MJPEG handler for HTTP streaming
    pub fn mjpeg_handler(&self) -> Arc<MjpegStreamHandler> {
        self.mjpeg_handler.clone()
    }

    /// Get WebSocket HID handler
    pub fn ws_hid_handler(&self) -> Arc<WsHidHandler> {
        self.ws_hid_handler.clone()
    }

    /// Get frame sender for WebRTC integration
    pub async fn frame_sender(&self) -> Option<broadcast::Sender<VideoFrame>> {
        if let Some(ref cap) = *self.capturer.read().await {
            Some(cap.frame_sender())
        } else {
            None
        }
    }

    // ========================================================================
    // Initialization
    // ========================================================================

    /// Initialize with auto-detected device
    pub async fn init_auto(self: &Arc<Self>) -> Result<()> {
        let best = find_best_device()?;
        self.init_with_device(best).await
    }

    /// Initialize with specific device
    pub async fn init_with_device(self: &Arc<Self>, device: VideoDeviceInfo) -> Result<()> {
        info!("MjpegStreamer: Initializing with device: {}", device.path.display());

        let config = self.config.read().await.clone();

        // Create capture config
        let capture_config = CaptureConfig {
            device_path: device.path.clone(),
            resolution: config.resolution,
            format: config.format,
            fps: config.fps,
            buffer_count: 4,
            timeout: std::time::Duration::from_secs(5),
            jpeg_quality: config.jpeg_quality,
        };

        // Create capturer
        let capturer = Arc::new(VideoCapturer::new(capture_config));

        // Store device and capturer
        *self.current_device.write().await = Some(device);
        *self.capturer.write().await = Some(capturer);
        *self.state.write().await = MjpegStreamerState::Ready;

        self.publish_state_change().await;
        Ok(())
    }

    // ========================================================================
    // Streaming Control
    // ========================================================================

    /// Start streaming
    pub async fn start(self: &Arc<Self>) -> Result<()> {
        let _lock = self.start_lock.lock().await;

        if self.config_changing.load(Ordering::SeqCst) {
            return Err(AppError::VideoError("Config change in progress".to_string()));
        }

        let state = *self.state.read().await;
        if state == MjpegStreamerState::Streaming {
            return Ok(());
        }

        // Get capturer
        let capturer = self.capturer.read().await.clone();
        let capturer = capturer.ok_or_else(|| AppError::VideoError("Not initialized".to_string()))?;

        // Start capture
        capturer.start().await?;

        // Start frame forwarding task
        let handler = self.mjpeg_handler.clone();
        let mut frame_rx = capturer.frame_sender().subscribe();
        tokio::spawn(async move {
            while let Ok(frame) = frame_rx.recv().await {
                handler.update_frame(frame);
            }
        });

        // Note: Audio WebSocket is handled separately by audio_ws.rs (/api/ws/audio)

        *self.state.write().await = MjpegStreamerState::Streaming;
        self.mjpeg_handler.set_online();

        self.publish_state_change().await;
        info!("MjpegStreamer: Streaming started");
        Ok(())
    }

    /// Stop streaming
    pub async fn stop(&self) -> Result<()> {
        let state = *self.state.read().await;
        if state != MjpegStreamerState::Streaming {
            return Ok(());
        }

        // Stop capturer
        if let Some(ref cap) = *self.capturer.read().await {
            let _ = cap.stop().await;
        }

        // Set offline
        self.mjpeg_handler.set_offline();
        *self.state.write().await = MjpegStreamerState::Ready;

        self.publish_state_change().await;
        info!("MjpegStreamer: Streaming stopped");
        Ok(())
    }

    /// Check if streaming
    pub async fn is_streaming(&self) -> bool {
        *self.state.read().await == MjpegStreamerState::Streaming
    }

    // ========================================================================
    // Configuration Updates
    // ========================================================================

    /// Apply video configuration
    ///
    /// This stops the current stream, reconfigures the capturer, and restarts.
    pub async fn apply_config(self: &Arc<Self>, config: MjpegStreamerConfig) -> Result<()> {
        info!("MjpegStreamer: Applying config: {:?}", config);

        self.config_changing.store(true, Ordering::SeqCst);

        // Stop current stream
        self.stop().await?;

        // Disconnect all MJPEG clients
        self.mjpeg_handler.disconnect_all_clients();

        // Release capturer
        *self.capturer.write().await = None;

        // Update config
        *self.config.write().await = config.clone();

        // Re-initialize if device path is set
        if let Some(ref path) = config.device_path {
            let devices = enumerate_devices()?;
            let device = devices
                .into_iter()
                .find(|d| d.path == *path)
                .ok_or_else(|| AppError::VideoError(format!("Device not found: {}", path.display())))?;

            self.init_with_device(device).await?;
        }

        self.config_changing.store(false, Ordering::SeqCst);
        self.publish_state_change().await;

        Ok(())
    }

    // ========================================================================
    // Internal
    // ========================================================================

    /// Publish state change event
    async fn publish_state_change(&self) {
        if let Some(ref events) = *self.events.read().await {
            let state = *self.state.read().await;
            let device = self.current_device.read().await;

            events.publish(SystemEvent::StreamStateChanged {
                state: state.to_string(),
                device: device.as_ref().map(|d| d.path.display().to_string()),
            });
        }
    }
}

impl Default for MjpegStreamer {
    fn default() -> Self {
        Self {
            config: RwLock::new(MjpegStreamerConfig::default()),
            capturer: RwLock::new(None),
            mjpeg_handler: Arc::new(MjpegStreamHandler::new()),
            current_device: RwLock::new(None),
            state: RwLock::new(MjpegStreamerState::Uninitialized),
            audio_controller: RwLock::new(None),
            audio_enabled: AtomicBool::new(false),
            ws_hid_handler: WsHidHandler::new(),
            hid_controller: RwLock::new(None),
            start_lock: tokio::sync::Mutex::new(()),
            events: RwLock::new(None),
            config_changing: AtomicBool::new(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mjpeg_streamer_creation() {
        let streamer = MjpegStreamer::new();
        assert!(!streamer.is_config_changing());
        assert!(!streamer.is_audio_enabled());
    }

    #[test]
    fn test_mjpeg_streamer_config_default() {
        let config = MjpegStreamerConfig::default();
        assert_eq!(config.resolution, Resolution::HD1080);
        assert_eq!(config.format, PixelFormat::Mjpeg);
        assert_eq!(config.fps, 30);
    }

    #[test]
    fn test_mjpeg_streamer_state_display() {
        assert_eq!(MjpegStreamerState::Streaming.to_string(), "streaming");
        assert_eq!(MjpegStreamerState::Ready.to_string(), "ready");
    }
}
