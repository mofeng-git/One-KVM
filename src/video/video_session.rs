//! Video session management with multi-codec support
//!
//! This module provides session management for video streaming with:
//! - Multi-codec support (H264, H265, VP8, VP9)
//! - Session lifecycle management
//! - Dynamic codec switching
//! - Statistics and monitoring

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{broadcast, RwLock};
use tracing::{debug, info, warn};

use super::encoder::registry::{EncoderBackend, EncoderRegistry, VideoEncoderType};
use super::format::Resolution;
use super::frame::VideoFrame;
use super::shared_video_pipeline::{
    EncodedVideoFrame, SharedVideoPipeline, SharedVideoPipelineConfig, SharedVideoPipelineStats,
};
use crate::error::{AppError, Result};

/// Maximum concurrent video sessions
const MAX_VIDEO_SESSIONS: usize = 8;

/// Video session state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoSessionState {
    /// Session created but not started
    Created,
    /// Session is active and streaming
    Active,
    /// Session is paused
    Paused,
    /// Session is closing
    Closing,
    /// Session is closed
    Closed,
}

impl std::fmt::Display for VideoSessionState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VideoSessionState::Created => write!(f, "Created"),
            VideoSessionState::Active => write!(f, "Active"),
            VideoSessionState::Paused => write!(f, "Paused"),
            VideoSessionState::Closing => write!(f, "Closing"),
            VideoSessionState::Closed => write!(f, "Closed"),
        }
    }
}

/// Video session information
#[derive(Debug, Clone)]
pub struct VideoSessionInfo {
    /// Session ID
    pub session_id: String,
    /// Current codec
    pub codec: VideoEncoderType,
    /// Session state
    pub state: VideoSessionState,
    /// Creation time
    pub created_at: Instant,
    /// Last activity time
    pub last_activity: Instant,
    /// Frames received
    pub frames_received: u64,
    /// Bytes received
    pub bytes_received: u64,
}

/// Individual video session
struct VideoSession {
    /// Session ID
    session_id: String,
    /// Codec for this session
    codec: VideoEncoderType,
    /// Session state
    state: VideoSessionState,
    /// Creation time
    created_at: Instant,
    /// Last activity time
    last_activity: Instant,
    /// Frame receiver
    frame_rx: Option<broadcast::Receiver<EncodedVideoFrame>>,
    /// Stats
    frames_received: u64,
    bytes_received: u64,
}

impl VideoSession {
    fn new(session_id: String, codec: VideoEncoderType) -> Self {
        let now = Instant::now();
        Self {
            session_id,
            codec,
            state: VideoSessionState::Created,
            created_at: now,
            last_activity: now,
            frame_rx: None,
            frames_received: 0,
            bytes_received: 0,
        }
    }

    fn info(&self) -> VideoSessionInfo {
        VideoSessionInfo {
            session_id: self.session_id.clone(),
            codec: self.codec,
            state: self.state,
            created_at: self.created_at,
            last_activity: self.last_activity,
            frames_received: self.frames_received,
            bytes_received: self.bytes_received,
        }
    }
}

/// Video session manager configuration
#[derive(Debug, Clone)]
pub struct VideoSessionManagerConfig {
    /// Default codec
    pub default_codec: VideoEncoderType,
    /// Default resolution
    pub resolution: Resolution,
    /// Default bitrate (kbps)
    pub bitrate_kbps: u32,
    /// Default FPS
    pub fps: u32,
    /// Session timeout (seconds)
    pub session_timeout_secs: u64,
    /// Encoder backend (None = auto select best available)
    pub encoder_backend: Option<EncoderBackend>,
}

impl Default for VideoSessionManagerConfig {
    fn default() -> Self {
        Self {
            default_codec: VideoEncoderType::H264,
            resolution: Resolution::HD720,
            bitrate_kbps: 8000,
            fps: 30,
            session_timeout_secs: 300,
            encoder_backend: None,
        }
    }
}

/// Video session manager
///
/// Manages video encoding sessions with multi-codec support.
/// A single encoder is shared across all sessions with the same codec.
pub struct VideoSessionManager {
    /// Configuration
    config: VideoSessionManagerConfig,
    /// Active sessions
    sessions: RwLock<HashMap<String, VideoSession>>,
    /// Current pipeline (shared across sessions with same codec)
    pipeline: RwLock<Option<Arc<SharedVideoPipeline>>>,
    /// Current codec (active pipeline codec)
    current_codec: RwLock<Option<VideoEncoderType>>,
    /// Video frame source
    frame_source: RwLock<Option<broadcast::Receiver<VideoFrame>>>,
}

impl VideoSessionManager {
    /// Create a new video session manager
    pub fn new(config: VideoSessionManagerConfig) -> Self {
        info!(
            "Creating video session manager with default codec: {}",
            config.default_codec
        );

        Self {
            config,
            sessions: RwLock::new(HashMap::new()),
            pipeline: RwLock::new(None),
            current_codec: RwLock::new(None),
            frame_source: RwLock::new(None),
        }
    }

    /// Create with default configuration
    pub fn with_defaults() -> Self {
        Self::new(VideoSessionManagerConfig::default())
    }

    /// Set the video frame source
    pub async fn set_frame_source(&self, rx: broadcast::Receiver<VideoFrame>) {
        *self.frame_source.write().await = Some(rx);
    }

    /// Get available codecs based on hardware capabilities
    pub fn available_codecs(&self) -> Vec<VideoEncoderType> {
        EncoderRegistry::global().selectable_formats()
    }

    /// Check if a codec is available
    pub fn is_codec_available(&self, codec: VideoEncoderType) -> bool {
        let hardware_only = codec.hardware_only();
        EncoderRegistry::global().is_format_available(codec, hardware_only)
    }

    /// Create a new video session
    pub async fn create_session(&self, codec: Option<VideoEncoderType>) -> Result<String> {
        let sessions = self.sessions.read().await;
        if sessions.len() >= MAX_VIDEO_SESSIONS {
            return Err(AppError::VideoError(format!(
                "Maximum video sessions ({}) reached",
                MAX_VIDEO_SESSIONS
            )));
        }
        drop(sessions);

        // Use specified codec or default
        let codec = codec.unwrap_or(self.config.default_codec);

        // Verify codec is available
        if !self.is_codec_available(codec) {
            return Err(AppError::VideoError(format!(
                "Codec {} is not available on this system",
                codec
            )));
        }

        // Generate session ID
        let session_id = uuid::Uuid::new_v4().to_string();

        // Create session
        let session = VideoSession::new(session_id.clone(), codec);

        // Store session
        let mut sessions = self.sessions.write().await;
        sessions.insert(session_id.clone(), session);

        info!(
            "Video session created: {} (codec: {})",
            session_id, codec
        );

        Ok(session_id)
    }

    /// Start a video session (subscribe to encoded frames)
    pub async fn start_session(
        &self,
        session_id: &str,
    ) -> Result<broadcast::Receiver<EncodedVideoFrame>> {
        // Ensure pipeline is running with correct codec
        self.ensure_pipeline_for_session(session_id).await?;

        let mut sessions = self.sessions.write().await;
        let session = sessions
            .get_mut(session_id)
            .ok_or_else(|| AppError::NotFound(format!("Session not found: {}", session_id)))?;

        // Get pipeline and subscribe
        let pipeline = self.pipeline.read().await;
        let pipeline = pipeline
            .as_ref()
            .ok_or_else(|| AppError::VideoError("Pipeline not initialized".to_string()))?;

        let rx = pipeline.subscribe();
        session.frame_rx = Some(pipeline.subscribe());
        session.state = VideoSessionState::Active;
        session.last_activity = Instant::now();

        info!("Video session started: {}", session_id);
        Ok(rx)
    }

    /// Ensure pipeline is running with correct codec for session
    async fn ensure_pipeline_for_session(&self, session_id: &str) -> Result<()> {
        let sessions = self.sessions.read().await;
        let session = sessions
            .get(session_id)
            .ok_or_else(|| AppError::NotFound(format!("Session not found: {}", session_id)))?;
        let required_codec = session.codec;
        drop(sessions);

        let current_codec = *self.current_codec.read().await;

        // Check if we need to create or switch pipeline
        if current_codec != Some(required_codec) {
            self.switch_pipeline_codec(required_codec).await?;
        }

        // Ensure pipeline is started
        let pipeline = self.pipeline.read().await;
        if let Some(ref pipe) = *pipeline {
            if !pipe.is_running() {
                // Need frame source to start
                let frame_rx = {
                    let source = self.frame_source.read().await;
                    source.as_ref().map(|rx| rx.resubscribe())
                };

                if let Some(rx) = frame_rx {
                    drop(pipeline);
                    let pipeline = self.pipeline.read().await;
                    if let Some(ref pipe) = *pipeline {
                        pipe.start(rx).await?;
                    }
                }
            }
        }

        Ok(())
    }

    /// Switch pipeline to different codec
    async fn switch_pipeline_codec(&self, codec: VideoEncoderType) -> Result<()> {
        info!("Switching pipeline to codec: {}", codec);

        // Stop existing pipeline
        {
            let pipeline = self.pipeline.read().await;
            if let Some(ref pipe) = *pipeline {
                pipe.stop();
            }
        }

        // Create new pipeline config
        let pipeline_config = SharedVideoPipelineConfig {
            resolution: self.config.resolution,
            input_format: crate::video::format::PixelFormat::Mjpeg, // Common input
            output_codec: codec,
            bitrate_kbps: self.config.bitrate_kbps,
            fps: self.config.fps,
            gop_size: 30,
            encoder_backend: self.config.encoder_backend,
        };

        // Create new pipeline
        let new_pipeline = SharedVideoPipeline::new(pipeline_config)?;

        // Update state
        *self.pipeline.write().await = Some(new_pipeline);
        *self.current_codec.write().await = Some(codec);

        info!("Pipeline switched to codec: {}", codec);
        Ok(())
    }

    /// Get session info
    pub async fn get_session(&self, session_id: &str) -> Option<VideoSessionInfo> {
        let sessions = self.sessions.read().await;
        sessions.get(session_id).map(|s| s.info())
    }

    /// List all sessions
    pub async fn list_sessions(&self) -> Vec<VideoSessionInfo> {
        let sessions = self.sessions.read().await;
        sessions.values().map(|s| s.info()).collect()
    }

    /// Pause a session
    pub async fn pause_session(&self, session_id: &str) -> Result<()> {
        let mut sessions = self.sessions.write().await;
        let session = sessions
            .get_mut(session_id)
            .ok_or_else(|| AppError::NotFound(format!("Session not found: {}", session_id)))?;

        session.state = VideoSessionState::Paused;
        session.last_activity = Instant::now();

        debug!("Video session paused: {}", session_id);
        Ok(())
    }

    /// Resume a session
    pub async fn resume_session(&self, session_id: &str) -> Result<()> {
        let mut sessions = self.sessions.write().await;
        let session = sessions
            .get_mut(session_id)
            .ok_or_else(|| AppError::NotFound(format!("Session not found: {}", session_id)))?;

        session.state = VideoSessionState::Active;
        session.last_activity = Instant::now();

        debug!("Video session resumed: {}", session_id);
        Ok(())
    }

    /// Close a session
    pub async fn close_session(&self, session_id: &str) -> Result<()> {
        let mut sessions = self.sessions.write().await;
        if let Some(mut session) = sessions.remove(session_id) {
            session.state = VideoSessionState::Closed;
            session.frame_rx = None;
            info!("Video session closed: {}", session_id);
        }

        // If no more sessions, consider stopping pipeline
        if sessions.is_empty() {
            drop(sessions);
            self.maybe_stop_pipeline().await;
        }

        Ok(())
    }

    /// Stop pipeline if no active sessions
    async fn maybe_stop_pipeline(&self) {
        let sessions = self.sessions.read().await;
        let has_active = sessions
            .values()
            .any(|s| s.state == VideoSessionState::Active);
        drop(sessions);

        if !has_active {
            let pipeline = self.pipeline.read().await;
            if let Some(ref pipe) = *pipeline {
                pipe.stop();
                debug!("Pipeline stopped - no active sessions");
            }
        }
    }

    /// Cleanup stale/timed out sessions
    pub async fn cleanup_stale_sessions(&self) {
        let timeout = std::time::Duration::from_secs(self.config.session_timeout_secs);
        let now = Instant::now();

        let stale_ids: Vec<String> = {
            let sessions = self.sessions.read().await;
            sessions
                .iter()
                .filter(|(_, s)| {
                    (s.state == VideoSessionState::Paused
                        || s.state == VideoSessionState::Created)
                        && now.duration_since(s.last_activity) > timeout
                })
                .map(|(id, _)| id.clone())
                .collect()
        };

        if !stale_ids.is_empty() {
            let mut sessions = self.sessions.write().await;
            for id in stale_ids {
                info!("Removing stale video session: {}", id);
                sessions.remove(&id);
            }
        }
    }

    /// Get session count
    pub async fn session_count(&self) -> usize {
        self.sessions.read().await.len()
    }

    /// Get active session count
    pub async fn active_session_count(&self) -> usize {
        self.sessions
            .read()
            .await
            .values()
            .filter(|s| s.state == VideoSessionState::Active)
            .count()
    }

    /// Get pipeline statistics
    pub async fn pipeline_stats(&self) -> Option<SharedVideoPipelineStats> {
        let pipeline = self.pipeline.read().await;
        if let Some(ref pipe) = *pipeline {
            Some(pipe.stats().await)
        } else {
            None
        }
    }

    /// Get current active codec
    pub async fn current_codec(&self) -> Option<VideoEncoderType> {
        *self.current_codec.read().await
    }

    /// Set bitrate for current pipeline
    pub async fn set_bitrate(&self, bitrate_kbps: u32) -> Result<()> {
        let pipeline = self.pipeline.read().await;
        if let Some(ref pipe) = *pipeline {
            pipe.set_bitrate(bitrate_kbps).await?;
        }
        Ok(())
    }

    /// Request keyframe for all sessions
    pub async fn request_keyframe(&self) {
        // This would be implemented if encoders support forced keyframes
        warn!("Keyframe request not yet implemented");
    }

    /// Change codec for a session (requires restart)
    pub async fn change_session_codec(
        &self,
        session_id: &str,
        new_codec: VideoEncoderType,
    ) -> Result<()> {
        if !self.is_codec_available(new_codec) {
            return Err(AppError::VideoError(format!(
                "Codec {} is not available",
                new_codec
            )));
        }

        let mut sessions = self.sessions.write().await;
        let session = sessions
            .get_mut(session_id)
            .ok_or_else(|| AppError::NotFound(format!("Session not found: {}", session_id)))?;

        let old_codec = session.codec;
        session.codec = new_codec;
        session.state = VideoSessionState::Created; // Require restart
        session.frame_rx = None;
        session.last_activity = Instant::now();

        info!(
            "Session {} codec changed: {} -> {}",
            session_id, old_codec, new_codec
        );

        Ok(())
    }

    /// Get codec info
    pub fn get_codec_info(&self, codec: VideoEncoderType) -> Option<CodecInfo> {
        let registry = EncoderRegistry::global();
        let encoder = registry.best_encoder(codec, codec.hardware_only())?;

        Some(CodecInfo {
            codec_type: codec,
            codec_name: encoder.codec_name.clone(),
            backend: encoder.backend.to_string(),
            is_hardware: encoder.is_hardware,
        })
    }

    /// List all available codecs with their info
    pub fn list_codec_info(&self) -> Vec<CodecInfo> {
        self.available_codecs()
            .iter()
            .filter_map(|c| self.get_codec_info(*c))
            .collect()
    }
}

/// Codec information
#[derive(Debug, Clone)]
pub struct CodecInfo {
    /// Codec type
    pub codec_type: VideoEncoderType,
    /// FFmpeg codec name
    pub codec_name: String,
    /// Backend (VAAPI, NVENC, etc.)
    pub backend: String,
    /// Whether this is hardware accelerated
    pub is_hardware: bool,
}

impl Default for VideoSessionManager {
    fn default() -> Self {
        Self::with_defaults()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_state_display() {
        assert_eq!(VideoSessionState::Active.to_string(), "Active");
        assert_eq!(VideoSessionState::Closed.to_string(), "Closed");
    }

    #[test]
    fn test_available_codecs() {
        let manager = VideoSessionManager::with_defaults();
        let codecs = manager.available_codecs();
        println!("Available codecs: {:?}", codecs);
        // H264 should always be available (software fallback)
        assert!(codecs.contains(&VideoEncoderType::H264));
    }

    #[test]
    fn test_codec_info() {
        let manager = VideoSessionManager::with_defaults();
        let info = manager.get_codec_info(VideoEncoderType::H264);
        if let Some(info) = info {
            println!(
                "H264: {} ({}, hardware={})",
                info.codec_name, info.backend, info.is_hardware
            );
        }
    }
}
