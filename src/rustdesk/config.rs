//! RustDesk Configuration
//!
//! Configuration types for the RustDesk protocol integration.

use serde::{Deserialize, Serialize};
use typeshare::typeshare;

/// RustDesk configuration
#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct RustDeskConfig {
    /// Enable RustDesk protocol
    pub enabled: bool,

    /// Rendezvous server address (hbbs), e.g., "rs.example.com" or "192.168.1.100"
    /// Port defaults to 21116 if not specified
    pub rendezvous_server: String,

    /// Relay server address (hbbr), if different from rendezvous server
    /// Usually the same host as rendezvous server but different port (21117)
    pub relay_server: Option<String>,

    /// Device ID (9-digit number), auto-generated if empty
    pub device_id: String,

    /// Device password for client authentication
    #[typeshare(skip)]
    pub device_password: String,

    /// Public key for encryption (Curve25519, base64 encoded), auto-generated
    #[typeshare(skip)]
    pub public_key: Option<String>,

    /// Private key for encryption (Curve25519, base64 encoded), auto-generated
    #[typeshare(skip)]
    pub private_key: Option<String>,

    /// Signing public key (Ed25519, base64 encoded), auto-generated
    /// Used for SignedId verification by clients
    #[typeshare(skip)]
    pub signing_public_key: Option<String>,

    /// Signing private key (Ed25519, base64 encoded), auto-generated
    /// Used for signing SignedId messages
    #[typeshare(skip)]
    pub signing_private_key: Option<String>,

    /// UUID for rendezvous server registration (persisted to avoid UUID_MISMATCH)
    #[typeshare(skip)]
    pub uuid: Option<String>,
}

impl Default for RustDeskConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            rendezvous_server: String::new(),
            relay_server: None,
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
    /// Check if the configuration is valid for starting the service
    pub fn is_valid(&self) -> bool {
        self.enabled
            && !self.rendezvous_server.is_empty()
            && !self.device_id.is_empty()
            && !self.device_password.is_empty()
    }

    /// Generate a new random device ID
    pub fn generate_device_id() -> String {
        generate_device_id()
    }

    /// Generate a new random password
    pub fn generate_password() -> String {
        generate_random_password()
    }

    /// Get or generate the UUID for rendezvous registration
    /// Returns (uuid_bytes, is_new) where is_new indicates if a new UUID was generated
    pub fn ensure_uuid(&mut self) -> ([u8; 16], bool) {
        if let Some(ref uuid_str) = self.uuid {
            // Try to parse existing UUID
            if let Ok(uuid) = uuid::Uuid::parse_str(uuid_str) {
                return (*uuid.as_bytes(), false);
            }
        }
        // Generate new UUID
        let new_uuid = uuid::Uuid::new_v4();
        self.uuid = Some(new_uuid.to_string());
        (*new_uuid.as_bytes(), true)
    }

    /// Get the UUID bytes (returns None if not set)
    pub fn get_uuid_bytes(&self) -> Option<[u8; 16]> {
        self.uuid.as_ref().and_then(|s| {
            uuid::Uuid::parse_str(s).ok().map(|u| *u.as_bytes())
        })
    }

    /// Get the rendezvous server address with default port
    pub fn rendezvous_addr(&self) -> String {
        if self.rendezvous_server.contains(':') {
            self.rendezvous_server.clone()
        } else {
            format!("{}:21116", self.rendezvous_server)
        }
    }

    /// Get the relay server address with default port
    pub fn relay_addr(&self) -> Option<String> {
        self.relay_server.as_ref().map(|s| {
            if s.contains(':') {
                s.clone()
            } else {
                format!("{}:21117", s)
            }
        }).or_else(|| {
            // Default: same host as rendezvous server
            if !self.rendezvous_server.is_empty() {
                let host = self.rendezvous_server.split(':').next().unwrap_or("");
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

/// Generate a random 9-digit device ID
pub fn generate_device_id() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let id: u32 = rng.gen_range(100_000_000..999_999_999);
    id.to_string()
}

/// Generate a random 8-character password
pub fn generate_random_password() -> String {
    use rand::Rng;
    const CHARSET: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    let mut rng = rand::thread_rng();
    (0..8)
        .map(|_| {
            let idx = rng.gen_range(0..CHARSET.len());
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
        let mut config = RustDeskConfig::default();

        config.rendezvous_server = "example.com".to_string();
        assert_eq!(config.rendezvous_addr(), "example.com:21116");

        config.rendezvous_server = "example.com:21116".to_string();
        assert_eq!(config.rendezvous_addr(), "example.com:21116");
    }

    #[test]
    fn test_relay_addr() {
        let mut config = RustDeskConfig::default();

        // No server configured
        assert!(config.relay_addr().is_none());

        // Rendezvous server configured, relay defaults to same host
        config.rendezvous_server = "example.com".to_string();
        assert_eq!(config.relay_addr(), Some("example.com:21117".to_string()));

        // Explicit relay server
        config.relay_server = Some("relay.example.com".to_string());
        assert_eq!(config.relay_addr(), Some("relay.example.com:21117".to_string()));
    }
}
