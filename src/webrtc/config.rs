use serde::{Deserialize, Serialize};

/// Public STUN from build-time secrets; TURN is user-configured.
pub mod public_ice {
    #[inline]
    pub fn is_configured() -> bool {
        true
    }

    #[inline]
    pub fn stun_server() -> &'static str {
        crate::secrets::ice::STUN_SERVER
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebRtcConfig {
    pub enabled: bool,
    pub stun_servers: Vec<String>,
    pub turn_servers: Vec<TurnServer>,
    pub enable_datachannel: bool,
    pub video_codec: VideoCodec,
    pub target_bitrate_kbps: u32,
    pub max_bitrate_kbps: u32,
    pub min_bitrate_kbps: u32,
    pub enable_audio: bool,
}

impl Default for WebRtcConfig {
    fn default() -> Self {
        Self {
            enabled: true,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnServer {
    pub urls: Vec<String>,
    pub username: String,
    pub credential: String,
}

impl TurnServer {
    pub fn new(url: String, username: String, credential: String) -> Self {
        Self {
            urls: vec![url],
            username,
            credential,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum VideoCodec {
    #[default]
    H264,
    VP8,
    VP9,
    AV1,
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

#[derive(Debug, Clone)]
pub struct IceConfig {
    pub gathering_timeout_ms: u64,
    pub connection_timeout_ms: u64,
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
