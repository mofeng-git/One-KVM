//! WebRTC configuration

use serde::{Deserialize, Serialize};

/// WebRTC configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebRtcConfig {
    /// Enable WebRTC
    pub enabled: bool,
    /// STUN server URLs
    pub stun_servers: Vec<String>,
    /// TURN server configuration
    pub turn_servers: Vec<TurnServer>,
    /// Enable DataChannel for HID
    pub enable_datachannel: bool,
    /// Video codec preference
    pub video_codec: VideoCodec,
    /// Target bitrate in kbps
    pub target_bitrate_kbps: u32,
    /// Maximum bitrate in kbps
    pub max_bitrate_kbps: u32,
    /// Minimum bitrate in kbps
    pub min_bitrate_kbps: u32,
    /// Enable audio track
    pub enable_audio: bool,
}

impl Default for WebRtcConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            // Empty STUN servers for local connections - host candidates work directly
            // For remote access, configure STUN/TURN servers via settings
            stun_servers: vec![],
            turn_servers: vec![],
            enable_datachannel: true,
            video_codec: VideoCodec::H264,
            target_bitrate_kbps: 1000,
            max_bitrate_kbps: 2000,
            min_bitrate_kbps: 500,
            enable_audio: true,
        }
    }
}

/// TURN server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnServer {
    /// TURN server URL (e.g., "turn:turn.example.com:3478")
    pub url: String,
    /// Username for TURN authentication
    pub username: String,
    /// Credential for TURN authentication
    pub credential: String,
}

/// Video codec preference
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VideoCodec {
    H264,
    VP8,
    VP9,
    AV1,
}

impl Default for VideoCodec {
    fn default() -> Self {
        Self::H264
    }
}

impl std::fmt::Display for VideoCodec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VideoCodec::H264 => write!(f, "H.264"),
            VideoCodec::VP8 => write!(f, "VP8"),
            VideoCodec::VP9 => write!(f, "VP9"),
            VideoCodec::AV1 => write!(f, "AV1"),
        }
    }
}

/// ICE configuration
#[derive(Debug, Clone)]
pub struct IceConfig {
    /// ICE candidate gathering timeout (ms)
    pub gathering_timeout_ms: u64,
    /// ICE connection timeout (ms)
    pub connection_timeout_ms: u64,
    /// Enable ICE lite mode
    pub ice_lite: bool,
}

impl Default for IceConfig {
    fn default() -> Self {
        Self {
            gathering_timeout_ms: 5000,
            connection_timeout_ms: 30000,
            ice_lite: false,
        }
    }
}
