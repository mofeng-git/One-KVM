use serde::{Deserialize, Serialize};
use typeshare::typeshare;

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AuthConfig {
    pub session_timeout_secs: u32,
    pub single_user_allow_multiple_sessions: bool,
    pub totp_enabled: bool,
    pub totp_secret: Option<String>,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            session_timeout_secs: 3600 * 24,
            single_user_allow_multiple_sessions: false,
            totp_enabled: false,
            totp_secret: None,
        }
    }
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct VideoConfig {
    pub device: Option<String>,
    pub format: Option<String>,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub quality: u32,
}

impl Default for VideoConfig {
    fn default() -> Self {
        Self {
            device: None,
            format: None,
            width: 1920,
            height: 1080,
            fps: 30,
            quality: 80,
        }
    }
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct MsdConfig {
    pub enabled: bool,
    pub msd_dir: String,
}

impl Default for MsdConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            msd_dir: String::new(),
        }
    }
}

impl MsdConfig {
    pub fn msd_dir_path(&self) -> std::path::PathBuf {
        std::path::PathBuf::from(&self.msd_dir)
    }

    pub fn images_dir(&self) -> std::path::PathBuf {
        self.msd_dir_path().join("images")
    }

    pub fn ventoy_dir(&self) -> std::path::PathBuf {
        self.msd_dir_path().join("ventoy")
    }

    pub fn drive_path(&self) -> std::path::PathBuf {
        self.ventoy_dir().join("ventoy.img")
    }
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioConfig {
    pub enabled: bool,
    pub device: String,
    pub quality: String,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            device: String::new(),
            quality: "balanced".to_string(),
        }
    }
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct WebConfig {
    pub http_port: u16,
    pub https_port: u16,
    pub bind_addresses: Vec<String>,
    pub bind_address: String,
    pub https_enabled: bool,
    pub ssl_cert_path: Option<String>,
    pub ssl_key_path: Option<String>,
}

impl Default for WebConfig {
    fn default() -> Self {
        Self {
            http_port: 8080,
            https_port: 8443,
            bind_addresses: Vec::new(),
            bind_address: "0.0.0.0".to_string(),
            https_enabled: false,
            ssl_cert_path: None,
            ssl_key_path: None,
        }
    }
}
