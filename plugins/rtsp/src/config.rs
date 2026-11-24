use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum RtspCodec {
    #[default]
    H264,
    H265,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct RtspConfig {
    pub enabled: bool,
    pub bind: String,
    pub port: u16,
    pub path: String,
    pub allow_one_client: bool,
    pub codec: RtspCodec,
    pub username: Option<String>,
    pub password: Option<String>,
}

impl Default for RtspConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            bind: "0.0.0.0".to_string(),
            port: 8554,
            path: "live".to_string(),
            allow_one_client: true,
            codec: RtspCodec::H264,
            username: None,
            password: None,
        }
    }
}
