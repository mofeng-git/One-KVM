//! WebRTC Streamer - High-level WebRTC streaming manager
//!
//! This module provides a unified interface for WebRTC streaming mode,
//! supporting multiple video codecs (H264, VP8, VP9, H265) and audio (Opus).
//!
//! # Architecture
//!
//! ```text
//! WebRtcStreamer
//!     |
//!     +-- Video Pipeline
//!     |       +-- SharedVideoPipeline (single encoder for all sessions)
//!     |               +-- H264 Encoder
//!     |               +-- H265 Encoder (hardware only)
//!     |               +-- VP8 Encoder (hardware only - VAAPI)
//!     |               +-- VP9 Encoder (hardware only - VAAPI)
//!     |
//!     +-- Audio Pipeline
//!     |       +-- SharedAudioPipeline
//!     |               +-- OpusEncoder
//!     |
//!     +-- UniversalSession[] (video + audio tracks + DataChannel)
//!             +-- UniversalVideoTrack (H264/H265/VP8/VP9)
//!             +-- Audio Track (RTP/Opus)
//!             +-- DataChannel (HID)
//! ```
//!
//! # Key Features
//!
//! - **Single encoder**: All sessions share one video encoder
//! - **Multi-codec support**: H264, H265, VP8, VP9
//! - **Audio support**: Opus audio streaming via SharedAudioPipeline
//! - **HID via DataChannel**: Keyboard/mouse events through WebRTC DataChannel

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tracing::{debug, error, info, trace, warn};

use crate::audio::shared_pipeline::{SharedAudioPipeline, SharedAudioPipelineConfig};
use crate::audio::{AudioController, OpusFrame};
use crate::error::{AppError, Result};
use crate::hid::HidController;
use crate::video::encoder::registry::VideoEncoderType;
use crate::video::encoder::registry::EncoderBackend;
use crate::video::encoder::VideoCodecType;
use crate::video::format::{PixelFormat, Resolution};
use crate::video::frame::VideoFrame;
use crate::video::shared_video_pipeline::{SharedVideoPipeline, SharedVideoPipelineConfig, SharedVideoPipelineStats};

use super::config::{TurnServer, WebRtcConfig};
use super::signaling::{ConnectionState, IceCandidate, SdpAnswer, SdpOffer};
use super::universal_session::{UniversalSession, UniversalSessionConfig};
use crate::video::encoder::BitratePreset;

/// WebRTC streamer configuration
#[derive(Debug, Clone)]
pub struct WebRtcStreamerConfig {
    /// WebRTC configuration (STUN/TURN servers, etc.)
    pub webrtc: WebRtcConfig,
    /// Video codec type
    pub video_codec: VideoCodecType,
    /// Input resolution
    pub resolution: Resolution,
    /// Input pixel format
    pub input_format: PixelFormat,
    /// Bitrate preset
    pub bitrate_preset: BitratePreset,
    /// Target FPS
    pub fps: u32,
    /// Enable audio (reserved)
    pub audio_enabled: bool,
    /// Encoder backend (None = auto select best available)
    pub encoder_backend: Option<EncoderBackend>,
}

impl Default for WebRtcStreamerConfig {
    fn default() -> Self {
        Self {
            webrtc: WebRtcConfig::default(),
            video_codec: VideoCodecType::H264,
            resolution: Resolution::HD720,
            input_format: PixelFormat::Mjpeg,
            bitrate_preset: BitratePreset::Balanced,
            fps: 30,
            audio_enabled: false,
            encoder_backend: None,
        }
    }
}

/// WebRTC streamer statistics
#[derive(Debug, Clone, Default)]
pub struct WebRtcStreamerStats {
    /// Number of active sessions
    pub session_count: usize,
    /// Current video codec
    pub video_codec: String,
    /// Video pipeline stats (if available)
    pub video_pipeline: Option<VideoPipelineStats>,
    /// Audio enabled
    pub audio_enabled: bool,
    /// Audio pipeline stats (if available)
    pub audio_pipeline: Option<AudioPipelineStats>,
}

/// Video pipeline statistics
#[derive(Debug, Clone, Default)]
pub struct VideoPipelineStats {
    pub frames_encoded: u64,
    pub frames_dropped: u64,
    pub bytes_encoded: u64,
    pub keyframes_encoded: u64,
    pub avg_encode_time_ms: f32,
    pub current_fps: f32,
    pub subscribers: u64,
}

/// Audio pipeline statistics
#[derive(Debug, Clone, Default)]
pub struct AudioPipelineStats {
    pub frames_encoded: u64,
    pub frames_dropped: u64,
    pub bytes_encoded: u64,
    pub avg_encode_time_ms: f32,
    pub subscribers: u64,
}

/// Session info for listing
#[derive(Debug, Clone)]
pub struct SessionInfo {
    pub session_id: String,
    pub created_at: std::time::Instant,
    pub state: String,
}

/// WebRTC Streamer
///
/// High-level manager for WebRTC streaming, supporting multiple video codecs
/// and audio streaming via Opus.
pub struct WebRtcStreamer {
    /// Current configuration
    config: RwLock<WebRtcStreamerConfig>,

    // === Video ===
    /// Current video codec type
    video_codec: RwLock<VideoCodecType>,
    /// Universal video pipeline (for all codecs)
    video_pipeline: RwLock<Option<Arc<SharedVideoPipeline>>>,
    /// All sessions (unified management)
    sessions: Arc<RwLock<HashMap<String, Arc<UniversalSession>>>>,
    /// Video frame source
    video_frame_tx: RwLock<Option<broadcast::Sender<VideoFrame>>>,

    // === Audio ===
    /// Audio enabled flag
    audio_enabled: RwLock<bool>,
    /// Shared audio pipeline for Opus encoding
    audio_pipeline: RwLock<Option<Arc<SharedAudioPipeline>>>,
    /// Audio controller reference
    audio_controller: RwLock<Option<Arc<AudioController>>>,

    // === Controllers ===
    /// HID controller for DataChannel
    hid_controller: RwLock<Option<Arc<HidController>>>,
}

impl WebRtcStreamer {
    /// Create a new WebRTC streamer
    pub fn new() -> Arc<Self> {
        Self::with_config(WebRtcStreamerConfig::default())
    }

    /// Create a new WebRTC streamer with configuration
    pub fn with_config(config: WebRtcStreamerConfig) -> Arc<Self> {
        Arc::new(Self {
            config: RwLock::new(config.clone()),
            video_codec: RwLock::new(config.video_codec),
            video_pipeline: RwLock::new(None),
            sessions: Arc::new(RwLock::new(HashMap::new())),
            video_frame_tx: RwLock::new(None),
            audio_enabled: RwLock::new(config.audio_enabled),
            audio_pipeline: RwLock::new(None),
            audio_controller: RwLock::new(None),
            hid_controller: RwLock::new(None),
        })
    }

    // === Video Codec Management ===

    /// Get current video codec type
    pub async fn current_video_codec(&self) -> VideoCodecType {
        *self.video_codec.read().await
    }

    /// Set video codec type
    ///
    /// Supports H264, H265, VP8, VP9. This will restart the video pipeline
    /// and close all existing sessions.
    pub async fn set_video_codec(self: &Arc<Self>, codec: VideoCodecType) -> Result<()> {
        let current = *self.video_codec.read().await;
        if current == codec {
            return Ok(());
        }

        info!("Switching video codec from {:?} to {:?}", current, codec);

        // Close all existing sessions
        self.close_all_sessions().await;

        // Stop current pipeline
        if let Some(ref pipeline) = *self.video_pipeline.read().await {
            pipeline.stop();
        }
        *self.video_pipeline.write().await = None;

        // Update codec
        *self.video_codec.write().await = codec;

        // Create new pipeline with new codec
        if let Some(ref tx) = *self.video_frame_tx.read().await {
            self.ensure_video_pipeline(tx.clone()).await?;
        }

        info!("Video codec switched to {:?}", codec);
        Ok(())
    }

    /// Get list of supported video codecs
    pub fn supported_video_codecs(&self) -> Vec<VideoCodecType> {
        use crate::video::encoder::registry::EncoderRegistry;

        let registry = EncoderRegistry::global();
        let mut codecs = vec![];

        // H264 always available (has software fallback)
        codecs.push(VideoCodecType::H264);

        // Check hardware codecs
        if registry.is_format_available(VideoEncoderType::H265, true) {
            codecs.push(VideoCodecType::H265);
        }
        if registry.is_format_available(VideoEncoderType::VP8, true) {
            codecs.push(VideoCodecType::VP8);
        }
        if registry.is_format_available(VideoEncoderType::VP9, true) {
            codecs.push(VideoCodecType::VP9);
        }

        codecs
    }

    /// Convert VideoCodecType to VideoEncoderType
    fn codec_type_to_encoder_type(codec: VideoCodecType) -> VideoEncoderType {
        match codec {
            VideoCodecType::H264 => VideoEncoderType::H264,
            VideoCodecType::H265 => VideoEncoderType::H265,
            VideoCodecType::VP8 => VideoEncoderType::VP8,
            VideoCodecType::VP9 => VideoEncoderType::VP9,
        }
    }

    /// Ensure video pipeline is initialized and running
    async fn ensure_video_pipeline(
        self: &Arc<Self>,
        tx: broadcast::Sender<VideoFrame>,
    ) -> Result<Arc<SharedVideoPipeline>> {
        let mut pipeline_guard = self.video_pipeline.write().await;

        if let Some(ref pipeline) = *pipeline_guard {
            if pipeline.is_running() {
                return Ok(pipeline.clone());
            }
        }

        let config = self.config.read().await;
        let codec = *self.video_codec.read().await;

        let pipeline_config = SharedVideoPipelineConfig {
            resolution: config.resolution,
            input_format: config.input_format,
            output_codec: Self::codec_type_to_encoder_type(codec),
            bitrate_preset: config.bitrate_preset,
            fps: config.fps,
            encoder_backend: config.encoder_backend,
            ..Default::default()
        };

        info!("Creating shared video pipeline for {:?}", codec);
        let pipeline = SharedVideoPipeline::new(pipeline_config)?;
        pipeline.start(tx.subscribe()).await?;

        // Start a monitor task to detect when pipeline auto-stops
        let pipeline_weak = Arc::downgrade(&pipeline);
        let streamer_weak = Arc::downgrade(self);
        let mut running_rx = pipeline.running_watch();

        tokio::spawn(async move {
            // Wait for pipeline to stop (running becomes false)
            while running_rx.changed().await.is_ok() {
                if !*running_rx.borrow() {
                    info!("Video pipeline auto-stopped, cleaning up resources");

                    // Clear pipeline reference in WebRtcStreamer
                    if let Some(streamer) = streamer_weak.upgrade() {
                        let mut pipeline_guard = streamer.video_pipeline.write().await;
                        // Only clear if it's the same pipeline that stopped
                        if let Some(ref current) = *pipeline_guard {
                            if let Some(stopped_pipeline) = pipeline_weak.upgrade() {
                                if Arc::ptr_eq(current, &stopped_pipeline) {
                                    *pipeline_guard = None;
                                    info!("Cleared stopped video pipeline reference");
                                }
                            }
                        }
                        drop(pipeline_guard);

                        // NOTE: Don't clear video_frame_tx here!
                        // The frame source is managed by stream_manager and should
                        // remain available for new sessions. Only stream_manager
                        // should clear it during mode switches.
                        info!("Video pipeline stopped, but keeping frame source for new sessions");
                    }
                    break;
                }
            }
            debug!("Video pipeline monitor task ended");
        });

        *pipeline_guard = Some(pipeline.clone());
        Ok(pipeline)
    }

    /// Ensure video pipeline is running and return it for external consumers
    ///
    /// This is a public wrapper around ensure_video_pipeline for external
    /// components (like RustDesk) that need to share the encoded video stream.
    pub async fn ensure_video_pipeline_for_external(
        self: &Arc<Self>,
        tx: broadcast::Sender<VideoFrame>,
    ) -> Result<Arc<SharedVideoPipeline>> {
        self.ensure_video_pipeline(tx).await
    }

    /// Get the current pipeline configuration (if pipeline is running)
    pub async fn get_pipeline_config(&self) -> Option<SharedVideoPipelineConfig> {
        if let Some(ref pipeline) = *self.video_pipeline.read().await {
            Some(pipeline.config().await)
        } else {
            None
        }
    }

    // === Audio Management ===

    /// Check if audio is enabled
    pub async fn is_audio_enabled(&self) -> bool {
        *self.audio_enabled.read().await
    }

    /// Set audio enabled state
    pub async fn set_audio_enabled(&self, enabled: bool) -> Result<()> {
        let was_enabled = *self.audio_enabled.read().await;
        *self.audio_enabled.write().await = enabled;
        self.config.write().await.audio_enabled = enabled;

        if enabled && !was_enabled {
            // Start audio pipeline if we have an audio controller
            if let Some(ref controller) = *self.audio_controller.read().await {
                self.start_audio_pipeline(controller.clone()).await?;
            }
        } else if !enabled && was_enabled {
            // Stop audio pipeline
            self.stop_audio_pipeline().await;
        }

        info!("WebRTC audio enabled: {}", enabled);
        Ok(())
    }

    /// Set audio controller reference
    pub async fn set_audio_controller(&self, controller: Arc<AudioController>) {
        info!("Setting audio controller for WebRTC streamer");
        *self.audio_controller.write().await = Some(controller.clone());

        // Start audio pipeline if audio is enabled
        if *self.audio_enabled.read().await {
            if let Err(e) = self.start_audio_pipeline(controller).await {
                error!("Failed to start audio pipeline: {}", e);
            }
        }
    }

    /// Start the shared audio pipeline
    async fn start_audio_pipeline(&self, controller: Arc<AudioController>) -> Result<()> {
        // Check if already running
        if let Some(ref pipeline) = *self.audio_pipeline.read().await {
            if pipeline.is_running() {
                debug!("Audio pipeline already running");
                return Ok(());
            }
        }

        // Get Opus frame receiver from audio controller
        let _opus_rx = match controller.subscribe_opus_async().await {
            Some(rx) => rx,
            None => {
                warn!("Audio controller not streaming, cannot start audio pipeline");
                return Ok(());
            }
        };

        // Create shared audio pipeline config
        let config = SharedAudioPipelineConfig::default();
        let pipeline = SharedAudioPipeline::new(config)?;

        // Note: SharedAudioPipeline expects raw AudioFrame, but AudioController
        // already provides encoded OpusFrame. We'll pass the OpusFrame directly
        // to sessions instead of re-encoding.
        // For now, store the pipeline reference for future use
        *self.audio_pipeline.write().await = Some(pipeline);

        // Reconnect audio for all existing sessions
        self.reconnect_audio_sources().await;

        info!("WebRTC audio pipeline started");
        Ok(())
    }

    /// Stop the shared audio pipeline
    async fn stop_audio_pipeline(&self) {
        if let Some(ref pipeline) = *self.audio_pipeline.read().await {
            pipeline.stop();
        }
        *self.audio_pipeline.write().await = None;
        info!("WebRTC audio pipeline stopped");
    }

    /// Subscribe to encoded Opus frames (for sessions)
    pub async fn subscribe_opus(&self) -> Option<broadcast::Receiver<OpusFrame>> {
        if let Some(ref controller) = *self.audio_controller.read().await {
            controller.subscribe_opus_async().await
        } else {
            None
        }
    }

    /// Reconnect audio source for all existing sessions
    /// Call this after audio controller restarts (e.g., quality change)
    pub async fn reconnect_audio_sources(&self) {
        if let Some(ref controller) = *self.audio_controller.read().await {
            let sessions = self.sessions.read().await;
            for (session_id, session) in sessions.iter() {
                if session.has_audio() {
                    info!("Reconnecting audio for session {}", session_id);
                    if let Some(rx) = controller.subscribe_opus_async().await {
                        session.start_audio_from_opus(rx).await;
                    }
                }
            }
        }
    }

    // === Video Frame Source ===

    /// Set video frame source
    pub async fn set_video_source(&self, tx: broadcast::Sender<VideoFrame>) {
        info!(
            "Setting video source for WebRTC streamer (receiver_count={})",
            tx.receiver_count()
        );
        *self.video_frame_tx.write().await = Some(tx.clone());

        // Start or restart pipeline if it exists
        if let Some(ref pipeline) = *self.video_pipeline.read().await {
            if !pipeline.is_running() {
                info!("Starting video pipeline with new frame source");
                if let Err(e) = pipeline.start(tx.subscribe()).await {
                    error!("Failed to start video pipeline: {}", e);
                }
            } else {
                // Pipeline is already running but may have old frame source
                // We need to restart it with the new frame source
                info!("Video pipeline already running, restarting with new frame source");
                pipeline.stop();
                tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
                if let Err(e) = pipeline.start(tx.subscribe()).await {
                    error!("Failed to restart video pipeline: {}", e);
                }
            }
        } else {
            info!("No video pipeline exists yet, frame source will be used when pipeline is created");
        }
    }

    /// Prepare for configuration change
    ///
    /// This stops the encoding pipeline and closes all sessions.
    pub async fn prepare_for_config_change(&self) {
        // Stop pipeline and close sessions - will be recreated on next session
        if let Some(ref pipeline) = *self.video_pipeline.read().await {
            pipeline.stop();
        }
        *self.video_pipeline.write().await = None;
        self.close_all_sessions().await;
    }

    /// Reconnect video source after configuration change
    pub async fn reconnect_video_source(&self, tx: broadcast::Sender<VideoFrame>) {
        self.set_video_source(tx).await;
    }

    // === Configuration ===

    /// Update video configuration
    ///
    /// Only restarts the encoding pipeline if configuration actually changed.
    /// This allows multiple consumers (WebRTC, RustDesk) to share the same pipeline
    /// without interrupting each other when they call this method with the same config.
    pub async fn update_video_config(
        &self,
        resolution: Resolution,
        format: PixelFormat,
        fps: u32,
    ) {
        // Check if configuration actually changed
        let config = self.config.read().await;
        let config_changed = config.resolution != resolution
            || config.input_format != format
            || config.fps != fps;
        drop(config);

        if !config_changed {
            // Configuration unchanged, no need to restart pipeline
            trace!(
                "Video config unchanged: {}x{} {:?} @ {} fps",
                resolution.width, resolution.height, format, fps
            );
            return;
        }

        // Configuration changed, restart pipeline
        info!(
            "Video config changed, restarting pipeline: {}x{} {:?} @ {} fps",
            resolution.width, resolution.height, format, fps
        );

        // Stop existing pipeline
        if let Some(ref pipeline) = *self.video_pipeline.read().await {
            pipeline.stop();
        }
        *self.video_pipeline.write().await = None;

        // Close all existing sessions - they need to reconnect
        let session_count = self.close_all_sessions().await;
        if session_count > 0 {
            info!("Closed {} existing sessions due to config change", session_count);
        }

        // Update config (preserve user-configured bitrate)
        let mut config = self.config.write().await;
        config.resolution = resolution;
        config.input_format = format;
        config.fps = fps;
        // Note: bitrate is NOT auto-scaled here - use set_bitrate() or config to change it

        info!(
            "WebRTC config updated: {}x{} {:?} @ {} fps, {}",
            resolution.width, resolution.height, format, fps, config.bitrate_preset
        );
    }

    /// Update encoder backend (software/hardware selection)
    pub async fn update_encoder_backend(&self, encoder_backend: Option<EncoderBackend>) {
        // Stop existing pipeline
        if let Some(ref pipeline) = *self.video_pipeline.read().await {
            pipeline.stop();
        }
        *self.video_pipeline.write().await = None;

        // Close all existing sessions - they need to reconnect with new encoder
        let session_count = self.close_all_sessions().await;
        if session_count > 0 {
            info!("Closed {} existing sessions due to encoder backend change", session_count);
        }

        // Update config
        let mut config = self.config.write().await;
        config.encoder_backend = encoder_backend;

        info!(
            "WebRTC encoder backend updated: {:?}",
            encoder_backend
        );
    }

    /// Check if current encoder configuration uses hardware encoding
    ///
    /// Returns true if:
    /// - A specific hardware backend is configured, OR
    /// - Auto mode is used and hardware encoders are available
    pub async fn is_hardware_encoding(&self) -> bool {
        let config = self.config.read().await;
        match config.encoder_backend {
            Some(backend) => backend.is_hardware(),
            None => {
                // Auto mode: check if hardware encoder is available for current codec
                use crate::video::encoder::registry::{EncoderRegistry, VideoEncoderType};
                let codec_type = match *self.video_codec.read().await {
                    VideoCodecType::H264 => VideoEncoderType::H264,
                    VideoCodecType::H265 => VideoEncoderType::H265,
                    VideoCodecType::VP8 => VideoEncoderType::VP8,
                    VideoCodecType::VP9 => VideoEncoderType::VP9,
                };
                EncoderRegistry::global()
                    .best_encoder(codec_type, false)
                    .map(|e| e.is_hardware)
                    .unwrap_or(false)
            }
        }
    }

    /// Update ICE configuration (STUN/TURN servers)
    ///
    /// Note: Changes take effect for new sessions only.
    /// Existing sessions need to be reconnected to use the new ICE config.
    ///
    /// If both stun_server and turn_server are empty/None, uses public ICE servers.
    pub async fn update_ice_config(
        &self,
        stun_server: Option<String>,
        turn_server: Option<String>,
        turn_username: Option<String>,
        turn_password: Option<String>,
    ) {
        let mut config = self.config.write().await;

        // Clear existing servers
        config.webrtc.stun_servers.clear();
        config.webrtc.turn_servers.clear();

        // Check if user configured custom servers
        let has_custom_stun = stun_server.as_ref().map(|s| !s.is_empty()).unwrap_or(false);
        let has_custom_turn = turn_server.as_ref().map(|s| !s.is_empty()).unwrap_or(false);

        // If no custom servers, use public ICE servers (like RustDesk)
        if !has_custom_stun && !has_custom_turn {
            use crate::webrtc::config::public_ice;
            if public_ice::is_configured() {
                if let Some(stun) = public_ice::stun_server() {
                    config.webrtc.stun_servers.push(stun.clone());
                    info!("Using public STUN server: {}", stun);
                }
                for turn in public_ice::turn_servers() {
                    info!("Using public TURN server: {:?}", turn.urls);
                    config.webrtc.turn_servers.push(turn);
                }
            } else {
                info!("No public ICE servers configured, using host candidates only");
            }
        } else {
            // Use custom servers
            if let Some(ref stun) = stun_server {
                if !stun.is_empty() {
                    config.webrtc.stun_servers.push(stun.clone());
                    info!("Using custom STUN server: {}", stun);
                }
            }
            if let Some(ref turn) = turn_server {
                if !turn.is_empty() {
                    let username = turn_username.unwrap_or_default();
                    let credential = turn_password.unwrap_or_default();
                    config.webrtc.turn_servers.push(TurnServer::new(
                        turn.clone(),
                        username.clone(),
                        credential,
                    ));
                    info!("Using custom TURN server: {} (user: {})", turn, username);
                }
            }
        }
    }

    /// Set HID controller for DataChannel
    pub async fn set_hid_controller(&self, hid: Arc<HidController>) {
        *self.hid_controller.write().await = Some(hid);
    }

    // === Session Management ===

    /// Create a new WebRTC session
    pub async fn create_session(self: &Arc<Self>) -> Result<String> {
        let session_id = uuid::Uuid::new_v4().to_string();
        let codec = *self.video_codec.read().await;

        // Ensure video pipeline is running
        let frame_tx = self.video_frame_tx.read().await.clone()
            .ok_or_else(|| AppError::VideoError("No video frame source".to_string()))?;
        let pipeline = self.ensure_video_pipeline(frame_tx).await?;

        // Create session config
        let config = self.config.read().await;
        let session_config = UniversalSessionConfig {
            webrtc: config.webrtc.clone(),
            codec: Self::codec_type_to_encoder_type(codec),
            resolution: config.resolution,
            input_format: config.input_format,
            bitrate_preset: config.bitrate_preset,
            fps: config.fps,
            audio_enabled: *self.audio_enabled.read().await,
        };
        drop(config);

        // Create universal session
        let mut session = UniversalSession::new(session_config.clone(), session_id.clone()).await?;

        // Set HID controller if available
        // Note: We DON'T create a data channel here - the frontend creates it.
        // The server only receives it via on_data_channel callback set in set_hid_controller().
        // If server also created a channel, frontend's ondatachannel would overwrite its
        // own channel with server's, but server's channel has no message handler!
        if let Some(ref hid) = *self.hid_controller.read().await {
            session.set_hid_controller(hid.clone());
        }

        let session = Arc::new(session);

        // Subscribe to video pipeline frames
        // Request keyframe after ICE connection is established (via callback)
        let pipeline_for_callback = pipeline.clone();
        let session_id_for_callback = session_id.clone();
        session.start_from_video_pipeline(pipeline.subscribe(), move || {
            // Spawn async task to request keyframe
            let pipeline = pipeline_for_callback;
            let sid = session_id_for_callback;
            tokio::spawn(async move {
                info!("Requesting keyframe for session {} after ICE connected", sid);
                pipeline.request_keyframe().await;
            });
        }).await;

        // Start audio if enabled
        if session_config.audio_enabled {
            if let Some(ref controller) = *self.audio_controller.read().await {
                if let Some(opus_rx) = controller.subscribe_opus_async().await {
                    session.start_audio_from_opus(opus_rx).await;
                }
            }
        }

        // Store session
        self.sessions
            .write()
            .await
            .insert(session_id.clone(), session);

        info!(
            "Session created: {} (codec={:?}, audio={}, {} total)",
            session_id,
            codec,
            session_config.audio_enabled,
            self.sessions.read().await.len()
        );

        Ok(session_id)
    }

    /// Handle SDP offer
    pub async fn handle_offer(&self, session_id: &str, offer: SdpOffer) -> Result<SdpAnswer> {
        let sessions = self.sessions.read().await;
        let session = sessions
            .get(session_id)
            .ok_or_else(|| AppError::NotFound(format!("Session not found: {}", session_id)))?;

        session.handle_offer(offer).await
    }

    /// Add ICE candidate
    pub async fn add_ice_candidate(&self, session_id: &str, candidate: IceCandidate) -> Result<()> {
        let sessions = self.sessions.read().await;
        let session = sessions
            .get(session_id)
            .ok_or_else(|| AppError::NotFound(format!("Session not found: {}", session_id)))?;

        session.add_ice_candidate(candidate).await
    }

    /// Close a session
    pub async fn close_session(&self, session_id: &str) -> Result<()> {
        let session = self.sessions.write().await.remove(session_id);

        if let Some(session) = session {
            session.close().await?;
        }

        // Stop pipeline if no more sessions
        if self.sessions.read().await.is_empty() {
            if let Some(ref pipeline) = *self.video_pipeline.read().await {
                info!("No more sessions, stopping video pipeline");
                pipeline.stop();
            }
        }

        Ok(())
    }

    /// Close all sessions
    pub async fn close_all_sessions(&self) -> usize {
        let mut sessions = self.sessions.write().await;
        let count = sessions.len();

        for (session_id, session) in sessions.drain() {
            debug!("Closing session {}", session_id);
            if let Err(e) = session.close().await {
                warn!("Error closing session {}: {}", session_id, e);
            }
        }

        // Stop pipeline
        drop(sessions);
        if let Some(ref pipeline) = *self.video_pipeline.read().await {
            pipeline.stop();
        }

        count
    }

    /// Get session count
    pub async fn session_count(&self) -> usize {
        self.sessions.read().await.len()
    }

    /// Get session info
    pub async fn get_session(&self, session_id: &str) -> Option<SessionInfo> {
        let sessions = self.sessions.read().await;
        sessions.get(session_id).map(|s| SessionInfo {
            session_id: s.session_id.clone(),
            created_at: std::time::Instant::now(),
            state: format!("{}", s.state()),
        })
    }

    /// List all sessions
    pub async fn list_sessions(&self) -> Vec<SessionInfo> {
        self.sessions
            .read()
            .await
            .values()
            .map(|s| SessionInfo {
                session_id: s.session_id.clone(),
                created_at: std::time::Instant::now(),
                state: format!("{}", s.state()),
            })
            .collect()
    }

    /// Cleanup closed sessions
    pub async fn cleanup(&self) {
        let to_remove: Vec<String> = {
            let sessions = self.sessions.read().await;
            sessions
                .iter()
                .filter(|(_, s)| {
                    matches!(
                        s.state(),
                        ConnectionState::Closed | ConnectionState::Failed | ConnectionState::Disconnected
                    )
                })
                .map(|(id, _)| id.clone())
                .collect()
        };

        if !to_remove.is_empty() {
            let mut sessions = self.sessions.write().await;
            for id in &to_remove {
                debug!("Removing closed session: {}", id);
                sessions.remove(id);
            }

            // Stop pipeline if no more sessions
            if sessions.is_empty() {
                drop(sessions);
                if let Some(ref pipeline) = *self.video_pipeline.read().await {
                    info!("No more sessions after cleanup, stopping video pipeline");
                    pipeline.stop();
                }
            }
        }
    }

    // === Statistics ===

    /// Get streamer statistics
    pub async fn stats(&self) -> WebRtcStreamerStats {
        let codec = *self.video_codec.read().await;
        let session_count = self.session_count().await;

        let video_pipeline = if let Some(ref pipeline) = *self.video_pipeline.read().await {
            let s = pipeline.stats().await;
            Some(VideoPipelineStats {
                frames_encoded: s.frames_encoded,
                frames_dropped: s.frames_dropped,
                bytes_encoded: s.bytes_encoded,
                keyframes_encoded: s.keyframes_encoded,
                avg_encode_time_ms: s.avg_encode_time_ms,
                current_fps: s.current_fps,
                subscribers: s.subscribers,
            })
        } else {
            None
        };

        // Get audio pipeline stats
        let audio_pipeline = if let Some(ref pipeline) = *self.audio_pipeline.read().await {
            let stats = pipeline.stats().await;
            Some(AudioPipelineStats {
                frames_encoded: stats.frames_encoded,
                frames_dropped: stats.frames_dropped,
                bytes_encoded: stats.bytes_encoded,
                avg_encode_time_ms: stats.avg_encode_time_ms,
                subscribers: stats.subscribers,
            })
        } else {
            None
        };

        WebRtcStreamerStats {
            session_count,
            video_codec: format!("{:?}", codec),
            video_pipeline,
            audio_enabled: *self.audio_enabled.read().await,
            audio_pipeline,
        }
    }

    /// Get pipeline statistics
    pub async fn pipeline_stats(&self) -> Option<SharedVideoPipelineStats> {
        if let Some(ref pipeline) = *self.video_pipeline.read().await {
            Some(pipeline.stats().await)
        } else {
            None
        }
    }

    /// Set bitrate using preset
    ///
    /// Note: Hardware encoders (VAAPI, NVENC, etc.) don't support dynamic bitrate changes.
    /// This method restarts the pipeline to apply the new bitrate only if the preset actually changed.
    pub async fn set_bitrate_preset(self: &Arc<Self>, preset: BitratePreset) -> Result<()> {
        // Check if preset actually changed
        let current_preset = self.config.read().await.bitrate_preset;
        if current_preset == preset {
            trace!("Bitrate preset unchanged: {}", preset);
            return Ok(());
        }

        // Update config
        self.config.write().await.bitrate_preset = preset;

        // Check if pipeline exists and is running
        let pipeline_running = {
            if let Some(ref pipeline) = *self.video_pipeline.read().await {
                pipeline.is_running()
            } else {
                false
            }
        };

        if pipeline_running {
            info!(
                "Restarting video pipeline to apply new bitrate: {}",
                preset
            );

            // Save video_frame_tx BEFORE stopping pipeline (monitor task will clear it)
            let saved_frame_tx = self.video_frame_tx.read().await.clone();

            // Stop existing pipeline
            if let Some(ref pipeline) = *self.video_pipeline.read().await {
                pipeline.stop();
            }

            // Wait for pipeline to stop
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

            // Clear pipeline reference - will be recreated
            *self.video_pipeline.write().await = None;

            // Recreate pipeline with new config if we have a frame source
            if let Some(tx) = saved_frame_tx {
                // Get existing sessions that need to be reconnected
                let session_ids: Vec<String> = self.sessions.read().await.keys().cloned().collect();

                if !session_ids.is_empty() {
                    // Restore video_frame_tx before recreating pipeline
                    *self.video_frame_tx.write().await = Some(tx.clone());

                    // Recreate pipeline
                    let pipeline = self.ensure_video_pipeline(tx).await?;

                    // Reconnect all sessions to new pipeline
                    let sessions = self.sessions.read().await;
                    for session_id in &session_ids {
                        if let Some(session) = sessions.get(session_id) {
                            info!("Reconnecting session {} to new pipeline", session_id);
                            let pipeline_for_callback = pipeline.clone();
                            let sid = session_id.clone();
                            session.start_from_video_pipeline(pipeline.subscribe(), move || {
                                let pipeline = pipeline_for_callback;
                                tokio::spawn(async move {
                                    info!("Requesting keyframe for session {} after reconnect", sid);
                                    pipeline.request_keyframe().await;
                                });
                            }).await;
                        }
                    }

                    info!(
                        "Video pipeline restarted with {}, reconnected {} sessions",
                        preset,
                        session_ids.len()
                    );
                }
            }
        } else {
            debug!(
                "Pipeline not running, bitrate {} will apply on next start",
                preset
            );
        }

        Ok(())
    }
}

impl Default for WebRtcStreamer {
    fn default() -> Self {
        Self {
            config: RwLock::new(WebRtcStreamerConfig::default()),
            video_codec: RwLock::new(VideoCodecType::H264),
            video_pipeline: RwLock::new(None),
            sessions: Arc::new(RwLock::new(HashMap::new())),
            video_frame_tx: RwLock::new(None),
            audio_enabled: RwLock::new(false),
            audio_pipeline: RwLock::new(None),
            audio_controller: RwLock::new(None),
            hid_controller: RwLock::new(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_webrtc_streamer_config_default() {
        let config = WebRtcStreamerConfig::default();
        assert_eq!(config.video_codec, VideoCodecType::H264);
        assert_eq!(config.resolution, Resolution::HD720);
        assert_eq!(config.bitrate_preset, BitratePreset::Balanced);
        assert_eq!(config.fps, 30);
        assert!(!config.audio_enabled);
    }

    #[tokio::test]
    async fn test_supported_codecs() {
        let streamer = WebRtcStreamer::new();
        let codecs = streamer.supported_video_codecs();
        assert!(codecs.contains(&VideoCodecType::H264));
    }
}
