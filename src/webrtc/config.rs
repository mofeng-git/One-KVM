use serde::{Deserialize, Serialize};
use tracing::info;
use webrtc::ice::udp_network::EphemeralUDP;

use crate::config::StreamConfig;
use crate::error::{AppError, Result};

/// Default public STUN server; TURN is user-configured.
pub mod public_ice {
    #[inline]
    pub fn is_configured() -> bool {
        true
    }

    #[inline]
    pub fn stun_server() -> &'static str {
        "stun:stun.l.google.com:19302"
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebRtcConfig {
    pub enabled: bool,
    #[serde(default)]
    pub udp_port_range: Option<String>,
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
            udp_port_range: None,
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

impl WebRtcConfig {
    pub(crate) fn apply_ice_settings(&mut self, config: &StreamConfig) {
        self.udp_port_range = config.webrtc_udp_port_range.clone();
        self.stun_servers.clear();
        self.turn_servers.clear();

        let stun = config.stun_server.as_ref().filter(|url| !url.is_empty());
        let turn = config.turn_server.as_ref().filter(|url| !url.is_empty());
        if stun.is_none() && turn.is_none() {
            let url = public_ice::stun_server();
            info!("Using public STUN server: {}", url);
            self.stun_servers.push(url.to_owned());
        }
        if let Some(url) = stun {
            info!("Using custom STUN server: {}", url);
            self.stun_servers.push(url.clone());
        }
        if let Some(url) = turn {
            let username = config.turn_username.clone().unwrap_or_default();
            let credential = config.turn_password.clone().unwrap_or_default();
            info!("Using custom TURN server: {} (user: {})", url, username);
            self.turn_servers
                .push(TurnServer::new(url.clone(), username, credential));
        }
    }
}

pub(crate) fn parse_udp_port_range(value: &str) -> Result<Option<EphemeralUDP>> {
    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }

    let invalid_range = || {
        AppError::BadRequest(
            "WebRTC UDP ports must be between 1 and 65535. Use a single port or an ascending range such as 50000-50100."
                .into(),
        )
    };
    let (min, max) = value.split_once('-').unwrap_or((value, value));
    let min = min.trim().parse::<u16>().map_err(|_| invalid_range())?;
    let max = max.trim().parse::<u16>().map_err(|_| invalid_range())?;
    if min == 0 {
        return Err(invalid_range());
    }

    EphemeralUDP::new(min, max)
        .map(Some)
        .map_err(|_| invalid_range())
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
