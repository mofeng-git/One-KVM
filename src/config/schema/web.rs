use serde::{Deserialize, Serialize};
use typeshare::typeshare;

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AuthConfig {
    pub session_timeout_secs: u32,
    pub single_user_allow_multiple_sessions: bool,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            session_timeout_secs: 3600 * 24,
            single_user_allow_multiple_sessions: false,
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
    pub sources: Vec<VideoSourceConfig>,
    pub primary_source_id: Option<String>,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct VideoSourceConfig {
    pub id: String,
    pub enabled: bool,
    pub format: String,
    pub quality: u32,
    pub label: String,
    pub device: String,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
}

impl Default for VideoSourceConfig {
    fn default() -> Self {
        Self {
            id: String::new(),
            enabled: true,
            format: "MJPEG".into(),
            quality: 80,
            label: String::new(),
            device: String::new(),
            width: 1920,
            height: 1080,
            fps: 30,
        }
    }
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
            sources: Vec::new(),
            primary_source_id: None,
        }
    }
}

impl VideoConfig {
    /// Legacy array positions are assigned once on load, then persisted with the entry.
    pub fn normalize_sources(&mut self) -> bool {
        let before = self.clone();
        let mut used: std::collections::HashSet<String> = self
            .sources
            .iter()
            .filter(|s| !s.id.is_empty())
            .map(|s| s.id.clone())
            .collect();
        for (index, source) in self.sources.iter_mut().enumerate() {
            if source.id.is_empty() {
                let mut number = index + 1;
                while used.contains(&number.to_string()) {
                    number += 1;
                }
                source.id = number.to_string();
                used.insert(source.id.clone());
            }
        }
        if self.primary_source_id.is_none() {
            self.primary_source_id = self.sources.first().map(|s| s.id.clone());
        }
        self.project_primary();
        *self != before
    }

    pub fn primary_id(&self) -> &str {
        self.primary_source_id.as_deref().unwrap_or("primary")
    }

    /// The legacy fields are a compatibility projection of the canonical source.
    pub fn project_primary(&mut self) {
        if let Some(source) = self
            .sources
            .iter()
            .find(|s| s.id == self.primary_id())
            .cloned()
        {
            self.device = Some(source.device);
            self.format = Some(source.format);
            self.width = source.width;
            self.height = source.height;
            self.fps = source.fps;
            self.quality = source.quality;
        }
    }

    pub fn effective_sources(&self) -> Vec<VideoSourceConfig> {
        if self.sources.is_empty() {
            vec![VideoSourceConfig {
                id: "primary".into(),
                label: "Primary".into(),
                device: self.device.clone().unwrap_or_else(|| "auto".into()),
                format: self.format.clone().unwrap_or_else(|| "MJPEG".into()),
                width: self.width,
                height: self.height,
                fps: self.fps,
                quality: self.quality,
                ..Default::default()
            }]
        } else {
            self.sources.clone()
        }
    }
}

#[typeshare]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MsdConfig {
    pub enabled: bool,
    pub msd_dir: String,
    pub flash_inquiry_string: String,
    pub cdrom_inquiry_string: String,
}

pub const DEFAULT_FLASH_INQUIRY_STRING: &str = "One-KVM Virtual Flash";
pub const DEFAULT_CDROM_INQUIRY_STRING: &str = "One-KVM Virtual CD-ROM";
pub const MAX_INQUIRY_STRING_BYTES: usize = 28;

impl Default for MsdConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            msd_dir: String::new(),
            flash_inquiry_string: DEFAULT_FLASH_INQUIRY_STRING.to_string(),
            cdrom_inquiry_string: DEFAULT_CDROM_INQUIRY_STRING.to_string(),
        }
    }
}

impl MsdConfig {
    pub fn validate(&self) -> crate::error::Result<()> {
        Self::validate_inquiry_string("Flash", &self.flash_inquiry_string)?;
        Self::validate_inquiry_string("CD-ROM", &self.cdrom_inquiry_string)
    }

    pub fn validate_inquiry_string(kind: &str, value: &str) -> crate::error::Result<()> {
        let value = value.trim();
        if value.is_empty() {
            return Err(crate::error::AppError::BadRequest(format!(
                "MSD {kind} inquiry string cannot be empty"
            )));
        }
        if value.len() > MAX_INQUIRY_STRING_BYTES {
            return Err(crate::error::AppError::BadRequest(format!(
                "MSD {kind} inquiry string must be at most {MAX_INQUIRY_STRING_BYTES} bytes"
            )));
        }
        if !value.bytes().all(|byte| (0x20..=0x7e).contains(&byte)) {
            return Err(crate::error::AppError::BadRequest(format!(
                "MSD {kind} inquiry string must contain printable ASCII characters only"
            )));
        }
        Ok(())
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_ids_migrate_once_and_survive_reorder() {
        let mut config: VideoConfig = serde_json::from_value(serde_json::json!({
            "sources": [{"device":"/dev/a"}, {"device":"/dev/b"}, {"device":"/dev/c"}]
        }))
        .unwrap();
        assert!(config.normalize_sources());
        assert_eq!(
            config
                .sources
                .iter()
                .map(|s| s.id.as_str())
                .collect::<Vec<_>>(),
            ["1", "2", "3"]
        );
        assert!(config
            .sources
            .iter()
            .all(|s| s.enabled && s.quality == 80 && s.format == "MJPEG"));
        config.sources.reverse();
        assert!(!config.normalize_sources());
        assert_eq!(config.primary_id(), "1");
        assert_eq!(config.device.as_deref(), Some("/dev/a"));
        let mut reloaded: VideoConfig =
            serde_json::from_str(&serde_json::to_string(&config).unwrap()).unwrap();
        assert!(!reloaded.normalize_sources());
        assert_eq!(reloaded.sources[0].id, "3");
        assert_eq!(reloaded.device, config.device);
    }

    #[test]
    fn msd_inquiry_strings_default_and_validate() {
        assert!(MsdConfig::default().validate().is_ok());
        assert!(MsdConfig::validate_inquiry_string("Flash", "  Custom Drive  ").is_ok());
        assert!(MsdConfig::validate_inquiry_string("Flash", "").is_err());
        assert!(MsdConfig::validate_inquiry_string("Flash", &"x".repeat(29)).is_err());
        assert!(MsdConfig::validate_inquiry_string("CD-ROM", "虚拟光驱").is_err());
        assert!(MsdConfig::validate_inquiry_string("CD-ROM", "bad\tname").is_err());
    }
}
