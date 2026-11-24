use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum VncEncoding {
    #[default]
    TightJpeg,
    H264,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct VncConfig {
    pub enabled: bool,
    pub bind: String,
    pub port: u16,
    pub encoding: VncEncoding,
    pub allow_one_client: bool,
    pub password: Option<String>,
}

impl Default for VncConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            bind: "0.0.0.0".to_string(),
            port: 5900,
            encoding: VncEncoding::TightJpeg,
            allow_one_client: true,
            password: None,
        }
    }
}
