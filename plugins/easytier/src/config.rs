use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EasytierConfigMode {
    Quick,
    Full,
}

impl Default for EasytierConfigMode {
    fn default() -> Self {
        Self::Quick
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct EasytierConfig {
    pub enabled: bool,
    pub config_mode: EasytierConfigMode,
    pub network_name: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub network_secret: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub peer_urls: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub virtual_ip: Option<String>,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub custom_toml: String,
}
