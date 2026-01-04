//! Video Stream Manager
//!
//! Unified manager for video streaming that supports single-mode operation.
//! At any given time, only one streaming mode (MJPEG or WebRTC) is active.
//!
//! # Architecture
//!
//! ```text
//! VideoStreamManager (Public API - Single Entry Point)
//!     │
//!     ├── mode: StreamMode (current active mode)
//!     │
//!     ├── MJPEG Mode
//!     │       └── Streamer ──► MjpegStreamHandler
//!     │           (Future: MjpegStreamer with WsAudio/WsHid)
//!     │
//!     └── WebRTC Mode
//!             └── WebRtcStreamer ──► H264SessionManager
//!                 (Extensible: H264, VP8, VP9, H265)
//! ```
//!
//! # Design Goals
//!
//! 1. **Single Entry Point**: All video operations go through VideoStreamManager
//! 2. **Mode Isolation**: MJPEG and WebRTC modes are cleanly separated
//! 3. **Extensible Codecs**: WebRTC supports multiple video codecs (H264 now, others reserved)
//! 4. **Simplified API**: Complex configuration flows are encapsulated

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use crate::config::{ConfigStore, StreamMode};
use crate::error::Result;
use crate::events::{EventBus, SystemEvent, VideoDeviceInfo};
use crate::hid::HidController;
use crate::stream::MjpegStreamHandler;
use crate::video::format::{PixelFormat, Resolution};
use crate::video::streamer::{Streamer, StreamerState};
use crate::webrtc::WebRtcStreamer;

/// Video stream manager configuration
#[derive(Debug, Clone)]
pub struct StreamManagerConfig {
    /// Initial streaming mode
    pub mode: StreamMode,
    /// Video device path
    pub device: Option<String>,
    /// Video format
    pub format: PixelFormat,
    /// Resolution
    pub resolution: Resolution,
    /// FPS
    pub fps: u32,
}

impl Default for StreamManagerConfig {
    fn default() -> Self {
        Self {
            mode: StreamMode::Mjpeg,
            device: None,
            format: PixelFormat::Mjpeg,
            resolution: Resolution::HD1080,
            fps: 30,
        }
    }
}

/// Unified video stream manager
///
/// Manages both MJPEG and WebRTC streaming modes, ensuring only one is active
/// at any given time. This reduces resource usage and simplifies the architecture.
///
/// # Components
///
/// - **Streamer**: Handles video capture and MJPEG distribution (current implementation)
/// - **WebRtcStreamer**: High-level WebRTC manager with multi-codec support (new)
/// - **H264SessionManager**: Legacy WebRTC manager (for backward compatibility)
pub struct VideoStreamManager {
    /// Current streaming mode
    mode: RwLock<StreamMode>,
    /// MJPEG streamer (handles video capture and MJPEG distribution)
    streamer: Arc<Streamer>,
    /// WebRTC streamer (unified WebRTC manager with multi-codec support)
    webrtc_streamer: Arc<WebRtcStreamer>,
    /// Event bus for notifications
    events: RwLock<Option<Arc<EventBus>>>,
    /// Configuration store
    config_store: RwLock<Option<ConfigStore>>,
    /// Mode switching lock to prevent concurrent switch requests
    switching: AtomicBool,
}

impl VideoStreamManager {
    /// Create a new video stream manager with WebRtcStreamer
    pub fn with_webrtc_streamer(
        streamer: Arc<Streamer>,
        webrtc_streamer: Arc<WebRtcStreamer>,
    ) -> Arc<Self> {
        Arc::new(Self {
            mode: RwLock::new(StreamMode::Mjpeg),
            streamer,
            webrtc_streamer,
            events: RwLock::new(None),
            config_store: RwLock::new(None),
            switching: AtomicBool::new(false),
        })
    }

    /// Check if mode switching is in progress
    pub fn is_switching(&self) -> bool {
        self.switching.load(Ordering::SeqCst)
    }

    /// Set event bus for notifications
    pub async fn set_event_bus(&self, events: Arc<EventBus>) {
        *self.events.write().await = Some(events);
    }

    /// Set configuration store
    pub async fn set_config_store(&self, config: ConfigStore) {
        *self.config_store.write().await = Some(config);
    }

    /// Get current streaming mode
    pub async fn current_mode(&self) -> StreamMode {
        self.mode.read().await.clone()
    }

    /// Check if MJPEG mode is active
    pub async fn is_mjpeg_enabled(&self) -> bool {
        *self.mode.read().await == StreamMode::Mjpeg
    }

    /// Check if WebRTC mode is active
    pub async fn is_webrtc_enabled(&self) -> bool {
        *self.mode.read().await == StreamMode::WebRTC
    }

    /// Get the underlying streamer (for MJPEG mode)
    pub fn streamer(&self) -> Arc<Streamer> {
        self.streamer.clone()
    }

    /// Get the WebRTC streamer (unified interface with multi-codec support)
    pub fn webrtc_streamer(&self) -> Arc<WebRtcStreamer> {
        self.webrtc_streamer.clone()
    }

    /// Get the MJPEG stream handler
    pub fn mjpeg_handler(&self) -> Arc<MjpegStreamHandler> {
        self.streamer.mjpeg_handler()
    }

    /// Initialize with a specific mode
    pub async fn init_with_mode(self: &Arc<Self>, mode: StreamMode) -> Result<()> {
        info!("Initializing video stream manager with mode: {:?}", mode);
        *self.mode.write().await = mode.clone();

        // Check if streamer is already initialized (capturer exists)
        let needs_init = self.streamer.state().await == StreamerState::Uninitialized;

        if needs_init {
            match mode {
                StreamMode::Mjpeg => {
                    // Initialize MJPEG streamer
                    if let Err(e) = self.streamer.init_auto().await {
                        warn!("Failed to auto-initialize MJPEG streamer: {}", e);
                    }
                }
                StreamMode::WebRTC => {
                    // WebRTC is initialized on-demand when clients connect
                    // But we still need to initialize the video capture
                    if let Err(e) = self.streamer.init_auto().await {
                        warn!("Failed to auto-initialize video capture for WebRTC: {}", e);
                    }
                }
            }
        }

        // Always reconnect frame source after initialization
        // This ensures WebRTC has the correct frame_tx from the current capturer
        if let Some(frame_tx) = self.streamer.frame_sender().await {
            // Synchronize WebRTC config with actual capture format
            let (format, resolution, fps) = self.streamer.current_video_config().await;
            info!(
                "Reconnecting frame source to WebRTC after init: {}x{} {:?} @ {}fps (receiver_count={})",
                resolution.width, resolution.height, format, fps, frame_tx.receiver_count()
            );
            self.webrtc_streamer.update_video_config(resolution, format, fps).await;
            self.webrtc_streamer.set_video_source(frame_tx).await;
        }

        Ok(())
    }

    /// Switch streaming mode
    ///
    /// This will:
    /// 1. Acquire switching lock (prevent concurrent switches)
    /// 2. Notify clients of the mode change
    /// 3. Stop the current mode
    /// 4. Start the new mode (ensuring video capture runs for WebRTC)
    /// 5. Update configuration
    pub async fn switch_mode(self: &Arc<Self>, new_mode: StreamMode) -> Result<()> {
        let current_mode = self.mode.read().await.clone();

        if current_mode == new_mode {
            debug!("Already in {:?} mode, no switch needed", new_mode);
            // Even if mode is the same, ensure video capture is running for WebRTC
            if new_mode == StreamMode::WebRTC {
                self.ensure_video_capture_running().await?;
            }
            return Ok(());
        }

        // Acquire switching lock - prevent concurrent switch requests
        if self.switching.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_err() {
            debug!("Mode switch already in progress, ignoring duplicate request");
            return Ok(());
        }

        // Use a helper to ensure we release the lock when done
        let result = self.do_switch_mode(current_mode, new_mode.clone()).await;
        self.switching.store(false, Ordering::SeqCst);
        result
    }

    /// Ensure video capture is running (for WebRTC mode)
    async fn ensure_video_capture_running(self: &Arc<Self>) -> Result<()> {
        // Initialize streamer if not already initialized
        if self.streamer.state().await == StreamerState::Uninitialized {
            info!("Initializing video capture for WebRTC (ensure)");
            if let Err(e) = self.streamer.init_auto().await {
                error!("Failed to initialize video capture: {}", e);
                return Err(e);
            }
        }

        // Start video capture if not streaming
        if self.streamer.state().await != StreamerState::Streaming {
            info!("Starting video capture for WebRTC (ensure)");
            if let Err(e) = self.streamer.start().await {
                error!("Failed to start video capture: {}", e);
                return Err(e);
            }

            // Wait a bit for capture to stabilize
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }

        // Reconnect frame source to WebRTC
        if let Some(frame_tx) = self.streamer.frame_sender().await {
            let (format, resolution, fps) = self.streamer.current_video_config().await;
            info!(
                "Reconnecting frame source to WebRTC: {}x{} {:?} @ {}fps",
                resolution.width, resolution.height, format, fps
            );
            self.webrtc_streamer.update_video_config(resolution, format, fps).await;
            self.webrtc_streamer.set_video_source(frame_tx).await;
        }

        Ok(())
    }

    /// Internal implementation of mode switching (called with lock held)
    async fn do_switch_mode(self: &Arc<Self>, current_mode: StreamMode, new_mode: StreamMode) -> Result<()> {
        info!("Switching video mode: {:?} -> {:?}", current_mode, new_mode);

        // Get the actual mode strings (with codec info for WebRTC)
        let new_mode_str = match &new_mode {
            StreamMode::Mjpeg => "mjpeg".to_string(),
            StreamMode::WebRTC => {
                let codec = self.webrtc_streamer.current_video_codec().await;
                codec_to_string(codec)
            }
        };
        let previous_mode_str = match &current_mode {
            StreamMode::Mjpeg => "mjpeg".to_string(),
            StreamMode::WebRTC => {
                let codec = self.webrtc_streamer.current_video_codec().await;
                codec_to_string(codec)
            }
        };

        // 1. Publish mode change event (clients should prepare to reconnect)
        self.publish_event(SystemEvent::StreamModeChanged {
            mode: new_mode_str,
            previous_mode: previous_mode_str,
        })
        .await;

        // 2. Stop current mode
        match current_mode {
            StreamMode::Mjpeg => {
                info!("Stopping MJPEG streaming");
                // Only stop MJPEG distribution, keep video capture running for WebRTC
                self.streamer.mjpeg_handler().set_offline();
                if let Err(e) = self.streamer.stop().await {
                    warn!("Error stopping MJPEG streamer: {}", e);
                }
            }
            StreamMode::WebRTC => {
                info!("Closing all WebRTC sessions");
                let closed = self.webrtc_streamer.close_all_sessions().await;
                if closed > 0 {
                    info!("Closed {} WebRTC sessions", closed);
                }
            }
        }

        // 3. Update mode
        *self.mode.write().await = new_mode.clone();

        // 4. Start new mode
        match new_mode {
            StreamMode::Mjpeg => {
                info!("Starting MJPEG streaming");

                // Auto-switch to MJPEG format if device supports it
                if let Some(device) = self.streamer.current_device().await {
                    let (current_format, resolution, fps) = self.streamer.current_video_config().await;
                    let available_formats: Vec<PixelFormat> = device.formats.iter().map(|f| f.format).collect();

                    // If current format is not MJPEG and device supports MJPEG, switch to it
                    if current_format != PixelFormat::Mjpeg && available_formats.contains(&PixelFormat::Mjpeg) {
                        info!("Auto-switching to MJPEG format for MJPEG mode");
                        let device_path = device.path.to_string_lossy().to_string();
                        if let Err(e) = self.streamer.apply_video_config(&device_path, PixelFormat::Mjpeg, resolution, fps).await {
                            warn!("Failed to auto-switch to MJPEG format: {}, keeping current format", e);
                        }
                    }
                }

                if let Err(e) = self.streamer.start().await {
                    error!("Failed to start MJPEG streamer: {}", e);
                    return Err(e);
                }
            }
            StreamMode::WebRTC => {
                // WebRTC mode: ensure video capture is running for H264 encoding
                info!("Activating WebRTC mode");

                // Initialize streamer if not already initialized
                if self.streamer.state().await == StreamerState::Uninitialized {
                    info!("Initializing video capture for WebRTC");
                    if let Err(e) = self.streamer.init_auto().await {
                        error!("Failed to initialize video capture for WebRTC: {}", e);
                        return Err(e);
                    }
                }

                // Auto-switch to non-compressed format if current format is MJPEG/JPEG
                if let Some(device) = self.streamer.current_device().await {
                    let (current_format, resolution, fps) = self.streamer.current_video_config().await;

                    if current_format.is_compressed() {
                        let available_formats: Vec<PixelFormat> = device.formats.iter().map(|f| f.format).collect();

                        // Determine if using hardware encoding
                        let is_hardware = self.webrtc_streamer.is_hardware_encoding().await;

                        if let Some(recommended) = PixelFormat::recommended_for_encoding(&available_formats, is_hardware) {
                            info!(
                                "Auto-switching from {:?} to {:?} for WebRTC encoding (hardware={})",
                                current_format, recommended, is_hardware
                            );
                            let device_path = device.path.to_string_lossy().to_string();
                            if let Err(e) = self.streamer.apply_video_config(&device_path, recommended, resolution, fps).await {
                                warn!("Failed to auto-switch format for WebRTC: {}, keeping current format", e);
                            }
                        }
                    }
                }

                // Start video capture if not streaming
                if self.streamer.state().await != StreamerState::Streaming {
                    info!("Starting video capture for WebRTC");
                    if let Err(e) = self.streamer.start().await {
                        error!("Failed to start video capture for WebRTC: {}", e);
                        return Err(e);
                    }
                }

                // Wait a bit for capture to stabilize
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

                // Connect frame source to WebRTC with correct format
                if let Some(frame_tx) = self.streamer.frame_sender().await {
                    // Synchronize WebRTC config with actual capture format
                    let (format, resolution, fps) = self.streamer.current_video_config().await;
                    info!(
                        "Connecting frame source to WebRTC pipeline: {}x{} {:?} @ {}fps",
                        resolution.width, resolution.height, format, fps
                    );
                    self.webrtc_streamer.update_video_config(resolution, format, fps).await;
                    self.webrtc_streamer.set_video_source(frame_tx).await;

                    // Get device path for events
                    let device_path = self.streamer.current_device().await
                        .map(|d| d.path.to_string_lossy().to_string())
                        .unwrap_or_default();

                    // Publish StreamConfigApplied event - clients can now safely connect
                    self.publish_event(SystemEvent::StreamConfigApplied {
                        device: device_path,
                        resolution: (resolution.width, resolution.height),
                        format: format!("{:?}", format).to_lowercase(),
                        fps,
                    })
                    .await;

                    // Publish WebRTCReady event - frame source is now connected
                    let codec = self.webrtc_streamer.current_video_codec().await;
                    let is_hardware = self.webrtc_streamer.is_hardware_encoding().await;
                    self.publish_event(SystemEvent::WebRTCReady {
                        codec: codec_to_string(codec),
                        hardware: is_hardware,
                    })
                    .await;
                } else {
                    warn!("No frame source available for WebRTC - sessions may fail to receive video");
                }

                info!("WebRTC mode activated (sessions created on-demand)");
            }
        }

        // 5. Update configuration store if available
        if let Some(ref config_store) = *self.config_store.read().await {
            let mut config = (*config_store.get()).clone();
            config.stream.mode = new_mode.clone();
            if let Err(e) = config_store.set(config).await {
                warn!("Failed to persist stream mode to config: {}", e);
            }
        }

        info!("Video mode switched to {:?}", new_mode);
        Ok(())
    }

    /// Apply video configuration (device, format, resolution, fps)
    ///
    /// This is called when video settings change. It will restart the
    /// appropriate streaming pipeline based on current mode.
    pub async fn apply_video_config(
        self: &Arc<Self>,
        device_path: &str,
        format: PixelFormat,
        resolution: Resolution,
        fps: u32,
    ) -> Result<()> {
        let mode = self.mode.read().await.clone();

        info!(
            "Applying video config: {} {:?} {}x{} @ {} fps (mode: {:?})",
            device_path, format, resolution.width, resolution.height, fps, mode
        );

        // Apply to streamer (handles video capture)
        self.streamer
            .apply_video_config(device_path, format, resolution, fps)
            .await?;

        // Update WebRTC config if in WebRTC mode
        if mode == StreamMode::WebRTC {
            self.webrtc_streamer
                .update_video_config(resolution, format, fps)
                .await;

            // Restart video capture for WebRTC (it was stopped during config change)
            info!("Restarting video capture for WebRTC after config change");
            if let Err(e) = self.streamer.start().await {
                error!("Failed to restart video capture for WebRTC: {}", e);
                return Err(e);
            }

            // Wait a bit for capture to stabilize
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

            // Reconnect frame source with the new capturer
            if let Some(frame_tx) = self.streamer.frame_sender().await {
                // Note: update_video_config was already called above with the requested config,
                // but verify that actual capture matches
                let (actual_format, actual_resolution, actual_fps) = self.streamer.current_video_config().await;
                if actual_format != format || actual_resolution != resolution || actual_fps != fps {
                    info!(
                        "Actual capture config differs from requested, updating WebRTC: {}x{} {:?} @ {}fps",
                        actual_resolution.width, actual_resolution.height, actual_format, actual_fps
                    );
                    self.webrtc_streamer.update_video_config(actual_resolution, actual_format, actual_fps).await;
                }
                info!("Reconnecting frame source to WebRTC after config change");
                self.webrtc_streamer.set_video_source(frame_tx).await;
            } else {
                warn!("No frame source available after config change");
            }
        }

        Ok(())
    }

    /// Start streaming (based on current mode)
    pub async fn start(self: &Arc<Self>) -> Result<()> {
        let mode = self.mode.read().await.clone();

        match mode {
            StreamMode::Mjpeg => {
                self.streamer.start().await?;
            }
            StreamMode::WebRTC => {
                // Ensure video capture is running
                if self.streamer.state().await == StreamerState::Uninitialized {
                    self.streamer.init_auto().await?;
                }
                if self.streamer.state().await != StreamerState::Streaming {
                    self.streamer.start().await?;
                }

                // Connect frame source with correct format
                if let Some(frame_tx) = self.streamer.frame_sender().await {
                    // Synchronize WebRTC config with actual capture format
                    let (format, resolution, fps) = self.streamer.current_video_config().await;
                    self.webrtc_streamer.update_video_config(resolution, format, fps).await;
                    self.webrtc_streamer.set_video_source(frame_tx).await;
                }
            }
        }

        Ok(())
    }

    /// Stop streaming
    pub async fn stop(&self) -> Result<()> {
        let mode = self.mode.read().await.clone();

        match mode {
            StreamMode::Mjpeg => {
                self.streamer.stop().await?;
            }
            StreamMode::WebRTC => {
                self.webrtc_streamer.close_all_sessions().await;
                self.streamer.stop().await?;
            }
        }

        Ok(())
    }

    /// Get video device info for device_info event
    pub async fn get_video_info(&self) -> VideoDeviceInfo {
        let stats = self.streamer.stats().await;
        let state = self.streamer.state().await;
        let device = self.streamer.current_device().await;
        let mode = self.mode.read().await.clone();

        // For WebRTC mode, return specific codec type (h264, h265, vp8, vp9)
        // instead of generic "webrtc" to prevent frontend from defaulting to h264
        let stream_mode = match &mode {
            StreamMode::Mjpeg => "mjpeg".to_string(),
            StreamMode::WebRTC => {
                let codec = self.webrtc_streamer.current_video_codec().await;
                codec_to_string(codec)
            }
        };

        VideoDeviceInfo {
            available: state != StreamerState::Uninitialized,
            device: device.map(|d| d.path.display().to_string()),
            format: stats.format,
            resolution: stats.resolution,
            fps: stats.target_fps,
            online: state == StreamerState::Streaming,
            stream_mode,
            config_changing: self.streamer.is_config_changing(),
            error: if state == StreamerState::Error {
                Some("Video stream error".to_string())
            } else if state == StreamerState::NoSignal {
                Some("No video signal".to_string())
            } else {
                None
            },
        }
    }

    /// Get MJPEG client count
    pub fn mjpeg_client_count(&self) -> u64 {
        self.streamer.mjpeg_handler().client_count()
    }

    /// Get WebRTC session count
    pub async fn webrtc_session_count(&self) -> usize {
        self.webrtc_streamer.session_count().await
    }

    /// Set HID controller for WebRTC DataChannel
    pub async fn set_hid_controller(&self, hid: Arc<HidController>) {
        self.webrtc_streamer.set_hid_controller(hid).await;
    }

    /// Set audio enabled state for WebRTC
    pub async fn set_webrtc_audio_enabled(&self, enabled: bool) -> Result<()> {
        self.webrtc_streamer.set_audio_enabled(enabled).await
    }

    /// Check if WebRTC audio is enabled
    pub async fn is_webrtc_audio_enabled(&self) -> bool {
        self.webrtc_streamer.is_audio_enabled().await
    }

    /// Reconnect audio sources for all WebRTC sessions
    /// Call this after audio controller restarts (e.g., quality change)
    pub async fn reconnect_webrtc_audio_sources(&self) {
        self.webrtc_streamer.reconnect_audio_sources().await;
    }

    // =========================================================================
    // Delegated methods from Streamer (for backward compatibility)
    // =========================================================================

    /// List available video devices
    pub async fn list_devices(&self) -> crate::error::Result<Vec<crate::video::device::VideoDeviceInfo>> {
        self.streamer.list_devices().await
    }

    /// Get streamer statistics
    pub async fn stats(&self) -> crate::video::streamer::StreamerStats {
        self.streamer.stats().await
    }

    /// Check if config is being changed
    pub fn is_config_changing(&self) -> bool {
        self.streamer.is_config_changing()
    }

    /// Check if streaming is active
    pub async fn is_streaming(&self) -> bool {
        self.streamer.is_streaming().await
    }

    /// Get frame sender for video frames
    pub async fn frame_sender(&self) -> Option<tokio::sync::broadcast::Sender<crate::video::frame::VideoFrame>> {
        self.streamer.frame_sender().await
    }

    /// Subscribe to encoded video frames from the shared video pipeline
    ///
    /// This allows RustDesk (and other consumers) to receive H264/H265/VP8/VP9
    /// encoded frames without running a separate encoder. The encoding is shared
    /// with WebRTC sessions.
    ///
    /// This method ensures video capture is running before subscribing.
    /// Returns None if video capture cannot be started or pipeline creation fails.
    pub async fn subscribe_encoded_frames(
        &self,
    ) -> Option<tokio::sync::broadcast::Receiver<crate::video::shared_video_pipeline::EncodedVideoFrame>> {
        // 1. Ensure video capture is initialized
        if self.streamer.state().await == StreamerState::Uninitialized {
            tracing::info!("Initializing video capture for encoded frame subscription");
            if let Err(e) = self.streamer.init_auto().await {
                tracing::error!("Failed to initialize video capture for encoded frames: {}", e);
                return None;
            }
        }

        // 2. Ensure video capture is running (streaming)
        if self.streamer.state().await != StreamerState::Streaming {
            tracing::info!("Starting video capture for encoded frame subscription");
            if let Err(e) = self.streamer.start().await {
                tracing::error!("Failed to start video capture for encoded frames: {}", e);
                return None;
            }
            // Wait for capture to stabilize
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }

        // 3. Get frame sender from running capture
        let frame_tx = match self.streamer.frame_sender().await {
            Some(tx) => tx,
            None => {
                tracing::warn!("Cannot subscribe to encoded frames: no frame sender available");
                return None;
            }
        };

        // 4. Synchronize WebRTC config with actual capture format
        let (format, resolution, fps) = self.streamer.current_video_config().await;
        tracing::info!(
            "Connecting encoded frame subscription: {}x{} {:?} @ {}fps",
            resolution.width, resolution.height, format, fps
        );
        self.webrtc_streamer.update_video_config(resolution, format, fps).await;

        // 5. Use WebRtcStreamer to ensure the shared video pipeline is running
        // This will create the pipeline if needed
        match self.webrtc_streamer.ensure_video_pipeline_for_external(frame_tx).await {
            Ok(pipeline) => Some(pipeline.subscribe()),
            Err(e) => {
                tracing::error!("Failed to start shared video pipeline: {}", e);
                None
            }
        }
    }

    /// Get the current video encoding configuration from the shared pipeline
    pub async fn get_encoding_config(&self) -> Option<crate::video::shared_video_pipeline::SharedVideoPipelineConfig> {
        self.webrtc_streamer.get_pipeline_config().await
    }

    /// Set video codec for the shared video pipeline
    ///
    /// This allows external consumers (like RustDesk) to set the video codec
    /// before subscribing to encoded frames.
    pub async fn set_video_codec(&self, codec: crate::video::encoder::VideoCodecType) -> crate::error::Result<()> {
        self.webrtc_streamer.set_video_codec(codec).await
    }

    /// Set bitrate preset for the shared video pipeline
    ///
    /// This allows external consumers (like RustDesk) to adjust the video quality
    /// based on client preferences.
    pub async fn set_bitrate_preset(&self, preset: crate::video::encoder::BitratePreset) -> crate::error::Result<()> {
        self.webrtc_streamer.set_bitrate_preset(preset).await
    }

    /// Publish event to event bus
    async fn publish_event(&self, event: SystemEvent) {
        if let Some(ref events) = *self.events.read().await {
            events.publish(event);
        }
    }
}

/// Convert VideoCodecType to lowercase string for frontend
fn codec_to_string(codec: crate::video::encoder::VideoCodecType) -> String {
    match codec {
        crate::video::encoder::VideoCodecType::H264 => "h264".to_string(),
        crate::video::encoder::VideoCodecType::H265 => "h265".to_string(),
        crate::video::encoder::VideoCodecType::VP8 => "vp8".to_string(),
        crate::video::encoder::VideoCodecType::VP9 => "vp9".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::video::encoder::VideoCodecType;

    #[test]
    fn test_codec_to_string() {
        assert_eq!(codec_to_string(VideoCodecType::H264), "h264");
        assert_eq!(codec_to_string(VideoCodecType::H265), "h265");
        assert_eq!(codec_to_string(VideoCodecType::VP8), "vp8");
        assert_eq!(codec_to_string(VideoCodecType::VP9), "vp9");
    }
}
