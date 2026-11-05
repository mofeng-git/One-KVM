use serde::{Deserialize, Serialize};
use typeshare::typeshare;

use super::software;

#[typeshare]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExtensionId {
    Ttyd,
    Gostc,
    Easyfrp,
    Easytier,
    Frpc,
}

impl ExtensionId {
    pub fn binary_path(&self) -> std::path::PathBuf {
        software::binary_path(*self)
    }

    pub fn all() -> &'static [ExtensionId] {
        &[
            Self::Ttyd,
            Self::Gostc,
            Self::Easyfrp,
            Self::Easytier,
            Self::Frpc,
        ]
    }
}

impl std::fmt::Display for ExtensionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ttyd => write!(f, "ttyd"),
            Self::Gostc => write!(f, "gostc"),
            Self::Easyfrp => write!(f, "easyfrp"),
            Self::Easytier => write!(f, "easytier"),
            Self::Frpc => write!(f, "frpc"),
        }
    }
}

impl std::str::FromStr for ExtensionId {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "ttyd" => Ok(Self::Ttyd),
            "gostc" => Ok(Self::Gostc),
            "easyfrp" => Ok(Self::Easyfrp),
            "easytier" => Ok(Self::Easytier),
            "frpc" => Ok(Self::Frpc),
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
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct EasyfrpConfig {
    pub enabled: bool,
    pub control_urls: Vec<String>,
}

#[typeshare]
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

#[typeshare]
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

#[typeshare]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FrpProxyType {
    Tcp,
    Udp,
    Http,
    Https,
    Stcp,
    Sudp,
    Xtcp,
}

impl Default for FrpProxyType {
    fn default() -> Self {
        Self::Tcp
    }
}

#[typeshare]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FrpcConfigMode {
    Quick,
    Full,
}

impl Default for FrpcConfigMode {
    fn default() -> Self {
        Self::Quick
    }
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct FrpcConfig {
    pub enabled: bool,
    pub config_mode: FrpcConfigMode,
    pub proxy_name: String,
    pub proxy_type: FrpProxyType,
    pub server_addr: String,
    pub server_port: u16,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub token: String,
    pub local_ip: String,
    pub local_port: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote_port: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_domain: Option<String>,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub secret_key: String,
    pub tls: bool,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub custom_toml: String,
}

impl Default for FrpcConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            config_mode: FrpcConfigMode::Quick,
            proxy_name: String::new(),
            proxy_type: FrpProxyType::Tcp,
            server_addr: String::new(),
            server_port: 7000,
            token: String::new(),
            local_ip: "127.0.0.1".to_string(),
            local_port: 22,
            remote_port: None,
            custom_domain: None,
            secret_key: String::new(),
            tls: true,
            custom_toml: String::new(),
        }
    }
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ExtensionsConfig {
    pub ttyd: TtydConfig,
    pub gostc: GostcConfig,
    pub easyfrp: EasyfrpConfig,
    pub easytier: EasytierConfig,
    pub frpc: FrpcConfig,
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
pub struct EasyfrpInfo {
    pub available: bool,
    pub status: ExtensionStatus,
    pub config: EasyfrpConfig,
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
pub struct FrpcInfo {
    pub available: bool,
    pub status: ExtensionStatus,
    pub config: FrpcConfig,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionsStatus {
    pub ttyd: TtydInfo,
    pub gostc: GostcInfo,
    pub easyfrp: EasyfrpInfo,
    pub easytier: EasytierInfo,
    pub frpc: FrpcInfo,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionLogs {
    pub id: ExtensionId,
    pub logs: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::{EasytierConfig, EasytierConfigMode, ExtensionsConfig};

    #[test]
    fn existing_easyfrp_config_leaves_gostc_disabled_with_tls_default() {
        let config: ExtensionsConfig = serde_json::from_str(
            r#"{"easyfrp":{"enabled":true,"control_urls":["wss://example.com/key"]}}"#,
        )
        .unwrap();
        assert!(!config.gostc.enabled);
        assert!(config.gostc.tls);
        assert!(config.easyfrp.enabled);
        assert_eq!(config.easyfrp.control_urls, ["wss://example.com/key"]);
    }

    #[test]
    fn gostc_and_easyfrp_configs_round_trip_independently() {
        let config: ExtensionsConfig = serde_json::from_str(
            r#"{"gostc":{"enabled":true,"addr":"old.example.com","key":"old-key","tls":false},"easyfrp":{"enabled":true,"control_urls":["wss://example.com/new-key"]}}"#,
        ).unwrap();
        let saved = serde_json::to_string(&config).unwrap();
        let restored: ExtensionsConfig = serde_json::from_str(&saved).unwrap();
        assert!(restored.gostc.enabled);
        assert!(!restored.gostc.tls);
        assert_eq!(restored.gostc.key, "old-key");
        assert!(restored.easyfrp.enabled);
        assert_eq!(restored.easyfrp.control_urls, ["wss://example.com/new-key"]);
    }

    #[test]
    fn legacy_gostc_configuration_is_preserved_independently_of_easyfrp() {
        let config: ExtensionsConfig = serde_json::from_str(
            r#"{"gostc":{"enabled":true,"addr":"old.example.com","key":"old-key","tls":true}}"#,
        )
        .unwrap();
        assert!(config.gostc.enabled);
        assert_eq!(config.gostc.addr, "old.example.com");
        assert_eq!(config.gostc.key, "old-key");
        assert!(config.gostc.tls);
        let saved = serde_json::to_value(&config).unwrap();
        assert_eq!(saved["gostc"]["key"], "old-key");
        assert!(!config.easyfrp.enabled);
        assert!(config.easyfrp.control_urls.is_empty());
    }

    #[test]
    fn legacy_easytier_config_defaults_to_quick_mode() {
        let config: EasytierConfig = serde_json::from_str(
            r#"{
                "enabled": true,
                "network_name": "legacy-network",
                "network_secret": "secret",
                "peer_urls": ["tcp://example.com:11010"],
                "virtual_ip": "10.10.10.2/24"
            }"#,
        )
        .expect("legacy EasyTier config should deserialize");

        assert_eq!(config.config_mode, EasytierConfigMode::Quick);
        assert!(config.custom_toml.is_empty());
        assert_eq!(config.network_name, "legacy-network");
    }
}
