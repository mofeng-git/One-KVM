//! Extension types and configurations

use serde::{Deserialize, Serialize};
use typeshare::typeshare;

/// Extension identifier (fixed set of supported extensions)
#[typeshare]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExtensionId {
    /// Web terminal (ttyd)
    Ttyd,
    /// NAT traversal client (gostc)
    Gostc,
    /// P2P VPN (easytier)
    Easytier,
}

impl ExtensionId {
    /// Get the binary path for this extension
    pub fn binary_path(&self) -> &'static str {
        match self {
            Self::Ttyd => "/usr/bin/ttyd",
            Self::Gostc => "/usr/bin/gostc",
            Self::Easytier => "/usr/bin/easytier-core",
        }
    }

    /// Get the display name for this extension
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Ttyd => "Web Terminal",
            Self::Gostc => "GOSTC Tunnel",
            Self::Easytier => "EasyTier VPN",
        }
    }

    /// Get all extension IDs
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

/// Extension running status
#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "state", content = "data", rename_all = "lowercase")]
pub enum ExtensionStatus {
    /// Binary not found at expected path
    Unavailable,
    /// Extension is stopped
    Stopped,
    /// Extension is running
    Running {
        /// Process ID
        pid: u32,
    },
    /// Extension failed to start
    Failed {
        /// Error message
        error: String,
    },
}

impl ExtensionStatus {
    pub fn is_running(&self) -> bool {
        matches!(self, Self::Running { .. })
    }
}

/// ttyd configuration (Web Terminal)
#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TtydConfig {
    /// Enable auto-start
    pub enabled: bool,
    /// Port to listen on
    pub port: u16,
    /// Shell to execute
    pub shell: String,
    /// Credential in format "user:password" (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credential: Option<String>,
}

impl Default for TtydConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            port: 7681,
            shell: "/bin/bash".to_string(),
            credential: None,
        }
    }
}

/// gostc configuration (NAT traversal based on FRP)
#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GostcConfig {
    /// Enable auto-start
    pub enabled: bool,
    /// Server address (e.g., gostc.mofeng.run)
    pub addr: String,
    /// Client key from GOSTC management panel
    #[serde(skip_serializing_if = "String::is_empty")]
    pub key: String,
    /// Enable TLS
    pub tls: bool,
}

impl Default for GostcConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            addr: "gostc.mofeng.run".to_string(),
            key: String::new(),
            tls: true,
        }
    }
}

/// EasyTier configuration (P2P VPN)
#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct EasytierConfig {
    /// Enable auto-start
    pub enabled: bool,
    /// Network name
    pub network_name: String,
    /// Network secret/password
    #[serde(skip_serializing_if = "String::is_empty")]
    pub network_secret: String,
    /// Peer node URLs
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub peer_urls: Vec<String>,
    /// Virtual IP address (optional, auto-assigned if not set)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub virtual_ip: Option<String>,
}

impl Default for EasytierConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            network_name: String::new(),
            network_secret: String::new(),
            peer_urls: Vec::new(),
            virtual_ip: None,
        }
    }
}

/// Combined extensions configuration
#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ExtensionsConfig {
    pub ttyd: TtydConfig,
    pub gostc: GostcConfig,
    pub easytier: EasytierConfig,
}

/// Extension info with status and config
#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionInfo {
    /// Whether binary exists
    pub available: bool,
    /// Current status
    pub status: ExtensionStatus,
}

/// ttyd extension info
#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TtydInfo {
    /// Whether binary exists
    pub available: bool,
    /// Current status
    pub status: ExtensionStatus,
    /// Configuration
    pub config: TtydConfig,
}

/// gostc extension info
#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GostcInfo {
    /// Whether binary exists
    pub available: bool,
    /// Current status
    pub status: ExtensionStatus,
    /// Configuration
    pub config: GostcConfig,
}

/// easytier extension info
#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EasytierInfo {
    /// Whether binary exists
    pub available: bool,
    /// Current status
    pub status: ExtensionStatus,
    /// Configuration
    pub config: EasytierConfig,
}

/// All extensions status response
#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionsStatus {
    pub ttyd: TtydInfo,
    pub gostc: GostcInfo,
    pub easytier: EasytierInfo,
}

/// Extension logs response
#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionLogs {
    pub id: ExtensionId,
    pub logs: Vec<String>,
}
