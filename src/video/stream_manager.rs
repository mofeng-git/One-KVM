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
use uuid::Uuid;

use crate::config::{ConfigStore, StreamMode};
use crate::error::Result;
use crate::events::{EventBus, SystemEvent, VideoDeviceInfo};
use crate::hid::HidController;
use crate::stream::MjpegStreamHandler;
use crate::video::codec_constraints::StreamCodecConstraints;
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

/// Result of a mode switch request.
#[derive(Debug, Clone)]
pub struct ModeSwitchTransaction {
    /// Whether this request started a new switch.
    pub accepted: bool,
    /// Whether a switch is currently in progress after handling this request.
    pub switching: bool,
    /// Transition ID if a switch is/was in progress.
    pub transition_id: Option<String>,
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
    /// Current mode switch transaction ID (set while switching=true)
    transition_id: RwLock<Option<String>>,
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
            transition_id: RwLock::new(None),
        })
    }

    /// Check if mode switching is in progress
    pub fn is_switching(&self) -> bool {
        self.switching.load(Ordering::SeqCst)
    }

    /// Get current mode switch transition ID, if any
    pub async fn current_transition_id(&self) -> Option<String> {
        self.transition_id.read().await.clone()
    }

    /// Set event bus for notifications
    pub async fn set_event_bus(&self, events: Arc<EventBus>) {
        *self.events.write().await = Some(events.clone());
        self.webrtc_streamer.set_event_bus(events).await;
    }

    /// Set configuration store
    pub async fn set_config_store(&self, config: ConfigStore) {
        *self.config_store.write().await = Some(config);
    }

    /// Get current stream codec constraints derived from global configuration.
    pub async fn codec_constraints(&self) -> StreamCodecConstraints {
        if let Some(ref config_store) = *self.config_store.read().await {
            let config = config_store.get();
            StreamCodecConstraints::from_config(&config)
        } else {
            StreamCodecConstraints::unrestricted()
        }
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

        // Configure WebRTC capture source after initialization
        let (device_path, resolution, format, fps, jpeg_quality) =
            self.streamer.current_capture_config().await;
        info!(
            "WebRTC capture config after init: {}x{} {:?} @ {}fps",
            resolution.width, resolution.height, format, fps
        );
        self.webrtc_streamer
            .update_video_config(resolution, format, fps)
            .await;
        if let Some(device_path) = device_path {
            self.webrtc_streamer
                .set_capture_device(device_path, jpeg_quality)
                .await;
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
        let _ = self.switch_mode_transaction(new_mode).await?;
        Ok(())
    }

    /// Switch streaming mode with a transaction ID for correlating events
    ///
    /// If a switch is already in progress, returns `accepted=false` with the
    /// current `transition_id` (if known) and does not start a new switch.
    pub async fn switch_mode_transaction(
        self: &Arc<Self>,
        new_mode: StreamMode,
    ) -> Result<ModeSwitchTransaction> {
        let current_mode = self.mode.read().await.clone();

        if current_mode == new_mode {
            debug!("Already in {:?} mode, no switch needed", new_mode);
            // Even if mode is the same, ensure video capture is running for WebRTC
            if new_mode == StreamMode::WebRTC {
                self.ensure_video_capture_running().await?;
            }
            return Ok(ModeSwitchTransaction {
                accepted: false,
                switching: false,
                transition_id: None,
            });
        }

        // Acquire switching lock - prevent concurrent switch requests
        if self
            .switching
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            debug!("Mode switch already in progress, ignoring duplicate request");
            return Ok(ModeSwitchTransaction {
                accepted: false,
                switching: true,
                transition_id: self.transition_id.read().await.clone(),
            });
        }

        let transition_id = Uuid::new_v4().to_string();
        *self.transition_id.write().await = Some(transition_id.clone());

        // Publish transaction start event
        let from_mode_str = self.mode_to_string(&current_mode).await;
        let to_mode_str = self.mode_to_string(&new_mode).await;
        self.publish_event(SystemEvent::StreamModeSwitching {
            transition_id: transition_id.clone(),
            to_mode: to_mode_str,
            from_mode: from_mode_str,
        })
        .await;

        // Perform the switch asynchronously so the HTTP handler can return
        // immediately and clients can reliably wait for WebSocket events.
        let manager = Arc::clone(self);
        let transition_id_for_task = transition_id.clone();
        tokio::spawn(async move {
            let result = manager
                .do_switch_mode(current_mode, new_mode, transition_id_for_task.clone())
                .await;

            if let Err(e) = result {
                error!(
                    "Mode switch transaction {} failed: {}",
                    transition_id_for_task, e
                );
            }

            // Publish transaction end marker with best-effort actual mode
            let actual_mode = manager.mode.read().await.clone();
            let actual_mode_str = manager.mode_to_string(&actual_mode).await;
            manager
                .publish_event(SystemEvent::StreamModeReady {
                    transition_id: transition_id_for_task.clone(),
                    mode: actual_mode_str,
                })
                .await;

            *manager.transition_id.write().await = None;
            manager.switching.store(false, Ordering::SeqCst);
        });

        Ok(ModeSwitchTransaction {
            accepted: true,
            switching: true,
            transition_id: Some(transition_id),
        })
    }

    async fn mode_to_string(&self, mode: &StreamMode) -> String {
        match mode {
            StreamMode::Mjpeg => "mjpeg".to_string(),
            StreamMode::WebRTC => {
                let codec = self.webrtc_streamer.current_video_codec().await;
                codec_to_string(codec)
            }
        }
    }

    /// Ensure video capture is running (for WebRTC mode)
    async fn ensure_video_capture_running(self: &Arc<Self>) -> Result<()> {
        // Initialize streamer if not already initialized (for config discovery)
        if self.streamer.state().await == StreamerState::Uninitialized {
            info!("Initializing video capture for WebRTC (ensure)");
            if let Err(e) = self.streamer.init_auto().await {
                error!("Failed to initialize video capture: {}", e);
                return Err(e);
            }
        }

        let (device_path, resolution, format, fps, jpeg_quality) =
            self.streamer.current_capture_config().await;
        info!(
            "Configuring WebRTC capture: {}x{} {:?} @ {}fps",
            resolution.width, resolution.height, format, fps
        );
        self.webrtc_streamer
            .update_video_config(resolution, format, fps)
            .await;
        if let Some(device_path) = device_path {
            self.webrtc_streamer
                .set_capture_device(device_path, jpeg_quality)
                .await;
        }

        Ok(())
    }

    /// Internal implementation of mode switching (called with lock held)
    async fn do_switch_mode(
        self: &Arc<Self>,
        current_mode: StreamMode,
        new_mode: StreamMode,
        transition_id: String,
    ) -> Result<()> {
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
            transition_id: Some(transition_id.clone()),
            mode: new_mode_str,
            previous_mode: previous_mode_str,
        })
        .await;

        // 2. Stop current mode
        match current_mode {
            StreamMode::Mjpeg => {
                info!("Stopping MJPEG streaming");
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
                    let (current_format, resolution, fps) =
                        self.streamer.current_video_config().await;
                    let available_formats: Vec<PixelFormat> =
                        device.formats.iter().map(|f| f.format).collect();

                    // If current format is not MJPEG and device supports MJPEG, switch to it
                    if current_format != PixelFormat::Mjpeg
                        && available_formats.contains(&PixelFormat::Mjpeg)
                    {
                        info!("Auto-switching to MJPEG format for MJPEG mode");
                        let device_path = device.path.to_string_lossy().to_string();
                        if let Err(e) = self
                            .streamer
                            .apply_video_config(&device_path, PixelFormat::Mjpeg, resolution, fps)
                            .await
                        {
                            warn!(
                                "Failed to auto-switch to MJPEG format: {}, keeping current format",
                                e
                            );
                        }
                    }
                }

                if let Err(e) = self.streamer.start().await {
                    error!("Failed to start MJPEG streamer: {}", e);
                    return Err(e);
                }
            }
            StreamMode::WebRTC => {
                // WebRTC mode: configure direct capture for encoder pipeline
                info!("Activating WebRTC mode");

                if self.streamer.state().await == StreamerState::Uninitialized {
                    info!("Initializing video capture for WebRTC");
                    if let Err(e) = self.streamer.init_auto().await {
                        error!("Failed to initialize video capture for WebRTC: {}", e);
                        return Err(e);
                    }
                }

                let (device_path, resolution, format, fps, jpeg_quality) =
                    self.streamer.current_capture_config().await;
                info!(
                    "Configuring WebRTC capture pipeline: {}x{} {:?} @ {}fps",
                    resolution.width, resolution.height, format, fps
                );
                self.webrtc_streamer
                    .update_video_config(resolution, format, fps)
                    .await;
                if let Some(device_path) = device_path {
                    self.webrtc_streamer
                        .set_capture_device(device_path, jpeg_quality)
                        .await;
                } else {
                    warn!("No capture device configured for WebRTC");
                }

                let codec = self.webrtc_streamer.current_video_codec().await;
                let is_hardware = self.webrtc_streamer.is_hardware_encoding().await;
                self.publish_event(SystemEvent::WebRTCReady {
                    transition_id: Some(transition_id.clone()),
                    codec: codec_to_string(codec),
                    hardware: is_hardware,
                })
                .await;

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

            let (device_path, actual_resolution, actual_format, actual_fps, jpeg_quality) =
                self.streamer.current_capture_config().await;
            if actual_format != format || actual_resolution != resolution || actual_fps != fps {
                info!(
                    "Actual capture config differs from requested, updating WebRTC: {}x{} {:?} @ {}fps",
                    actual_resolution.width, actual_resolution.height, actual_format, actual_fps
                );
                self.webrtc_streamer
                    .update_video_config(actual_resolution, actual_format, actual_fps)
                    .await;
            }
            if let Some(device_path) = device_path {
                info!("Configuring direct capture for WebRTC after config change");
                self.webrtc_streamer
                    .set_capture_device(device_path, jpeg_quality)
                    .await;
            } else {
                warn!("No capture device configured for WebRTC after config change");
            }

            let codec = self.webrtc_streamer.current_video_codec().await;
            let is_hardware = self.webrtc_streamer.is_hardware_encoding().await;
            self.publish_event(SystemEvent::WebRTCReady {
                transition_id: None,
                codec: codec_to_string(codec),
                hardware: is_hardware,
            })
            .await;
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
                // Ensure device is initialized for config discovery
                if self.streamer.state().await == StreamerState::Uninitialized {
                    self.streamer.init_auto().await?;
                }

                // Synchronize WebRTC config with current capture config
                let (device_path, resolution, format, fps, jpeg_quality) =
                    self.streamer.current_capture_config().await;
                self.webrtc_streamer
                    .update_video_config(resolution, format, fps)
                    .await;
                if let Some(device_path) = device_path {
                    self.webrtc_streamer
                        .set_capture_device(device_path, jpeg_quality)
                        .await;
                } else {
                    warn!("No capture device configured for WebRTC");
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
    pub async fn list_devices(
        &self,
    ) -> crate::error::Result<Vec<crate::video::device::VideoDeviceInfo>> {
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
    ) -> Option<
        tokio::sync::mpsc::Receiver<
            std::sync::Arc<crate::video::shared_video_pipeline::EncodedVideoFrame>,
        >,
    > {
        // 1. Ensure video capture is initialized (for config discovery)
        if self.streamer.state().await == StreamerState::Uninitialized {
            tracing::info!("Initializing video capture for encoded frame subscription");
            if let Err(e) = self.streamer.init_auto().await {
                tracing::error!(
                    "Failed to initialize video capture for encoded frames: {}",
                    e
                );
                return None;
            }
        }

        // 2. Synchronize WebRTC config with capture config
        let (device_path, resolution, format, fps, jpeg_quality) =
            self.streamer.current_capture_config().await;
        tracing::info!(
            "Connecting encoded frame subscription: {}x{} {:?} @ {}fps",
            resolution.width,
            resolution.height,
            format,
            fps
        );
        self.webrtc_streamer
            .update_video_config(resolution, format, fps)
            .await;
        if let Some(device_path) = device_path {
            self.webrtc_streamer
                .set_capture_device(device_path, jpeg_quality)
                .await;
        } else {
            tracing::warn!("No capture device configured for encoded frames");
            return None;
        }

        // 3. Use WebRtcStreamer to ensure the shared video pipeline is running
        match self
            .webrtc_streamer
            .ensure_video_pipeline_for_external()
            .await
        {
            Ok(pipeline) => Some(pipeline.subscribe()),
            Err(e) => {
                tracing::error!("Failed to start shared video pipeline: {}", e);
                None
            }
        }
    }

    /// Get the current video encoding configuration from the shared pipeline
    pub async fn get_encoding_config(
        &self,
    ) -> Option<crate::video::shared_video_pipeline::SharedVideoPipelineConfig> {
        self.webrtc_streamer.get_pipeline_config().await
    }

    /// Set video codec for the shared video pipeline
    ///
    /// This allows external consumers (like RustDesk) to set the video codec
    /// before subscribing to encoded frames.
    pub async fn set_video_codec(
        &self,
        codec: crate::video::encoder::VideoCodecType,
    ) -> crate::error::Result<()> {
        self.webrtc_streamer.set_video_codec(codec).await
    }

    /// Set bitrate preset for the shared video pipeline
    ///
    /// This allows external consumers (like RustDesk) to adjust the video quality
    /// based on client preferences.
    pub async fn set_bitrate_preset(
        &self,
        preset: crate::video::encoder::BitratePreset,
    ) -> crate::error::Result<()> {
        self.webrtc_streamer.set_bitrate_preset(preset).await
    }

    /// Request a keyframe from the shared video pipeline
    pub async fn request_keyframe(&self) -> crate::error::Result<()> {
        self.webrtc_streamer.request_keyframe().await
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
