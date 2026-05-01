use serde::{Deserialize, Serialize};
use typeshare::typeshare;

#[typeshare]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExtensionId {
    Ttyd,
    Gostc,
    Easytier,
}

impl ExtensionId {
    pub fn binary_path(&self) -> &'static str {
        match self {
            Self::Ttyd => "/usr/bin/ttyd",
            Self::Gostc => "/usr/bin/gostc",
            Self::Easytier => "/usr/bin/easytier-core",
        }
    }

    pub fn all() -> &'static [ExtensionId] {
        &[Self::Ttyd, Self::Gostc, Self::Easytier]
    }
}

impl std::fmt::Display for ExtensionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ttyd => write!(f, "ttyd"),
            Self::Gostc => write!(f, "gostc"),
            Self::Easytier => write!(f, "easytier"),
        }
    }
}

impl std::str::FromStr for ExtensionId {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "ttyd" => Ok(Self::Ttyd),
            "gostc" => Ok(Self::Gostc),
            "easytier" => Ok(Self::Easytier),
            _ => Err(format!("Unknown extension: {}", s)),
        }
    }
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "state", content = "data", rename_all = "lowercase")]
pub enum ExtensionStatus {
    Unavailable,
    Stopped,
    Running { pid: u32 },
    Failed { error: String },
}

impl ExtensionStatus {
    pub fn is_running(&self) -> bool {
        matches!(self, Self::Running { .. })
    }
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TtydConfig {
    pub enabled: bool,
    pub shell: String,
}

impl Default for TtydConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            shell: "/bin/bash".to_string(),
        }
    }
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GostcConfig {
    pub enabled: bool,
    pub addr: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub key: String,
    pub tls: bool,
}

impl Default for GostcConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            addr: String::new(),
            key: String::new(),
            tls: true,
        }
    }
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
#[derive(Default)]
pub struct EasytierConfig {
    pub enabled: bool,
    pub network_name: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub network_secret: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub peer_urls: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub virtual_ip: Option<String>,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ExtensionsConfig {
    pub ttyd: TtydConfig,
    pub gostc: GostcConfig,
    pub easytier: EasytierConfig,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionInfo {
    pub available: bool,
    pub status: ExtensionStatus,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TtydInfo {
    pub available: bool,
    pub status: ExtensionStatus,
    pub config: TtydConfig,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GostcInfo {
    pub available: bool,
    pub status: ExtensionStatus,
    pub config: GostcConfig,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EasytierInfo {
    pub available: bool,
    pub status: ExtensionStatus,
    pub config: EasytierConfig,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionsStatus {
    pub ttyd: TtydInfo,
    pub gostc: GostcInfo,
    pub easytier: EasytierInfo,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionLogs {
    pub id: ExtensionId,
    pub logs: Vec<String>,
}
