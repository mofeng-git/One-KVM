use serde::{Deserialize, Serialize};
use typeshare::typeshare;

#[typeshare]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum RustDeskCodec {
    #[default]
    H264,
    H265,
}

#[typeshare]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum RustDeskMode {
    #[default]
    Id,
    DirectIp,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct RustDeskConfig {
    pub enabled: bool,
    pub mode: RustDeskMode,
    pub codec: RustDeskCodec,
    pub direct_access_port: u16,
    pub rendezvous_server: String,
    pub relay_server: Option<String>,
    #[typeshare(skip)]
    pub relay_key: Option<String>,
    pub device_id: String,
    #[typeshare(skip)]
    pub device_password: String,
    #[typeshare(skip)]
    pub public_key: Option<String>,
    #[typeshare(skip)]
    pub private_key: Option<String>,
    #[typeshare(skip)]
    pub signing_public_key: Option<String>,
    #[typeshare(skip)]
    pub signing_private_key: Option<String>,
    #[typeshare(skip)]
    pub uuid: Option<String>,
}

impl Default for RustDeskConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            mode: RustDeskMode::Id,
            codec: RustDeskCodec::H264,
            direct_access_port: 21118,
            rendezvous_server: String::new(),
            relay_server: None,
            relay_key: None,
            device_id: generate_device_id(),
            device_password: generate_random_password(),
            public_key: None,
            private_key: None,
            signing_public_key: None,
            signing_private_key: None,
            uuid: None,
        }
    }
}

impl RustDeskConfig {
    pub fn is_valid(&self) -> bool {
        self.enabled
            && !self.device_id.is_empty()
            && !self.device_password.is_empty()
            && match self.mode {
                RustDeskMode::Id => !self.rendezvous_server.trim().is_empty(),
                RustDeskMode::DirectIp => self.direct_access_port != 0,
            }
    }

    pub fn effective_rendezvous_server(&self) -> &str {
        &self.rendezvous_server
    }

    pub fn generate_device_id() -> String {
        generate_device_id()
    }

    pub fn generate_password() -> String {
        generate_random_password()
    }

    pub fn ensure_uuid(&mut self) -> ([u8; 16], bool) {
        if let Some(ref uuid_str) = self.uuid {
            if let Ok(uuid) = uuid::Uuid::parse_str(uuid_str) {
                return (*uuid.as_bytes(), false);
            }
        }
        let new_uuid = uuid::Uuid::new_v4();
        self.uuid = Some(new_uuid.to_string());
        (*new_uuid.as_bytes(), true)
    }

    pub fn get_uuid_bytes(&self) -> Option<[u8; 16]> {
        self.uuid
            .as_ref()
            .and_then(|s| uuid::Uuid::parse_str(s).ok().map(|u| *u.as_bytes()))
    }

    pub fn rendezvous_addr(&self) -> String {
        let server = &self.rendezvous_server;
        if server.is_empty() {
            String::new()
        } else if server.contains(':') {
            server.to_string()
        } else {
            format!("{}:21116", server)
        }
    }

    pub fn relay_addr(&self) -> Option<String> {
        self.relay_server
            .as_ref()
            .map(|s| {
                if s.contains(':') {
                    s.clone()
                } else {
                    format!("{}:21117", s)
                }
            })
            .or_else(|| {
                let server = &self.rendezvous_server;
                if !server.is_empty() {
                    let host = server.split(':').next().unwrap_or("");
                    if !host.is_empty() {
                        Some(format!("{}:21117", host))
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
    }
}

pub fn generate_device_id() -> String {
    use rand::RngExt;
    let mut rng = rand::rng();
    let id: u32 = rng.random_range(100_000_000..999_999_999);
    id.to_string()
}

pub fn generate_random_password() -> String {
    use rand::RngExt;
    const CHARSET: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    let mut rng = rand::rng();
    (0..8)
        .map(|_| {
            let idx = rng.random_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_device_id_generation() {
        let id = generate_device_id();
        assert_eq!(id.len(), 9);
        assert!(id.chars().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn test_password_generation() {
        let password = generate_random_password();
        assert_eq!(password.len(), 8);
        assert!(password.chars().all(|c| c.is_alphanumeric()));
    }

    #[test]
    fn test_rendezvous_addr() {
        let mut config = RustDeskConfig {
            rendezvous_server: "example.com".to_string(),
            ..Default::default()
        };

        assert_eq!(config.rendezvous_addr(), "example.com:21116");

        config.rendezvous_server = "example.com:21116".to_string();
        assert_eq!(config.rendezvous_addr(), "example.com:21116");

        config.rendezvous_server = String::new();
        assert_eq!(config.rendezvous_addr(), "");
    }

    #[test]
    fn test_relay_addr() {
        let mut config = RustDeskConfig {
            rendezvous_server: "example.com".to_string(),
            ..Default::default()
        };

        assert_eq!(config.relay_addr(), Some("example.com:21117".to_string()));

        config.relay_server = Some("relay.example.com".to_string());
        assert_eq!(
            config.relay_addr(),
            Some("relay.example.com:21117".to_string())
        );

        config.rendezvous_server = String::new();
        config.relay_server = None;
        assert_eq!(config.relay_addr(), None);
    }

    #[test]
    fn test_effective_rendezvous_server() {
        let mut config = RustDeskConfig {
            rendezvous_server: "custom.example.com".to_string(),
            ..Default::default()
        };

        assert_eq!(config.effective_rendezvous_server(), "custom.example.com");

        config.rendezvous_server = String::new();
        assert_eq!(config.effective_rendezvous_server(), "");
    }

    #[test]
    fn direct_ip_mode_is_valid_without_rendezvous_server() {
        let config = RustDeskConfig {
            enabled: true,
            mode: RustDeskMode::DirectIp,
            rendezvous_server: String::new(),
            ..Default::default()
        };

        assert!(config.is_valid());
    }

    #[test]
    fn id_mode_is_invalid_without_rendezvous_server() {
        let config = RustDeskConfig {
            enabled: true,
            mode: RustDeskMode::Id,
            rendezvous_server: String::new(),
            ..Default::default()
        };

        assert!(!config.is_valid());
    }

    #[test]
    fn legacy_config_defaults_to_id_mode() {
        let config: RustDeskConfig = serde_json::from_value(serde_json::json!({
            "enabled": false,
            "codec": "h264",
            "rendezvous_server": "",
            "device_id": "123456789",
            "device_password": "password"
        }))
        .expect("legacy RustDesk config should deserialize");

        assert_eq!(config.mode, RustDeskMode::Id);
        assert_eq!(config.direct_access_port, 21118);
    }
}
