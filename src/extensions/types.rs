use serde::{Deserialize, Serialize};
use typeshare::typeshare;

use super::software;

#[typeshare]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExtensionId {
    Ttyd,
}

impl ExtensionId {
    pub fn compiled(&self) -> bool {
        match self {
            Self::Ttyd => cfg!(feature = "terminal"),
        }
    }

    pub fn binary_path(&self) -> std::path::PathBuf {
        software::binary_path(*self)
    }

    pub fn all() -> &'static [ExtensionId] {
        &[Self::Ttyd]
    }
}

impl std::fmt::Display for ExtensionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ttyd => write!(f, "ttyd"),
        }
    }
}

impl std::str::FromStr for ExtensionId {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "ttyd" => Ok(Self::Ttyd),
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
            shell: software::default_ttyd_shell().to_string(),
        }
    }
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ExtensionsConfig {
    pub ttyd: TtydConfig,
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
pub struct ExtensionsStatus {
    pub ttyd: TtydInfo,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionLogs {
    pub id: ExtensionId,
    pub logs: Vec<String>,
}
