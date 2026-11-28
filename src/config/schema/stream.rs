use serde::{Deserialize, Serialize};
use typeshare::typeshare;

use super::BitratePreset;

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum StreamMode {
    WebRTC,
    #[default]
    Mjpeg,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum EncoderType {
    #[default]
    Auto,
    Software,
    Vaapi,
    Nvenc,
    Qsv,
    Amf,
    Rkmpp,
    #[serde(alias = "amlogic")]
    V4l2m2m,
}

impl EncoderType {
    pub fn display_name(&self) -> &'static str {
        match self {
            EncoderType::Auto => "Auto (Recommended)",
            EncoderType::Software => "Software (CPU)",
            EncoderType::Vaapi => "VAAPI",
            EncoderType::Nvenc => "NVIDIA NVENC",
            EncoderType::Qsv => "Intel Quick Sync",
            EncoderType::Amf => "AMD AMF",
            EncoderType::Rkmpp => "Rockchip MPP",
            EncoderType::V4l2m2m => "V4L2 M2M",
        }
    }
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct StreamConfig {
    pub mode: StreamMode,
    pub keep_capture_alive: bool,
    pub encoder: EncoderType,
    pub bitrate_preset: BitratePreset,
    pub webrtc_udp_port_range: Option<String>,
    pub stun_server: Option<String>,
    pub turn_server: Option<String>,
    pub turn_username: Option<String>,
    pub turn_password: Option<String>,
    #[typeshare(skip)]
    pub auto_pause_enabled: bool,
    #[typeshare(skip)]
    pub auto_pause_delay_secs: u64,
    #[typeshare(skip)]
    pub client_timeout_secs: u64,
}

impl Default for StreamConfig {
    fn default() -> Self {
        Self {
            mode: StreamMode::Mjpeg,
            keep_capture_alive: false,
            encoder: EncoderType::Auto,
            bitrate_preset: BitratePreset::Balanced,
            webrtc_udp_port_range: None,
            stun_server: None,
            turn_server: None,
            turn_username: None,
            turn_password: None,
            auto_pause_enabled: false,
            auto_pause_delay_secs: 10,
            client_timeout_secs: 30,
        }
    }
}

impl StreamConfig {
    pub fn is_using_public_ice_servers(&self) -> bool {
        let no_custom_stun = self
            .stun_server
            .as_ref()
            .map_or(true, |s| s.trim().is_empty());
        let no_custom_turn = self
            .turn_server
            .as_ref()
            .map_or(true, |s| s.trim().is_empty());
        no_custom_stun && no_custom_turn
    }
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct RedfishConfig {
    pub enabled: bool,
}

impl Default for RedfishConfig {
    fn default() -> Self {
        Self { enabled: false }
    }
}
