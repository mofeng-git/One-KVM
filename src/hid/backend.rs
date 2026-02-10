//! HID backend trait definition

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use super::types::{ConsumerEvent, KeyboardEvent, MouseEvent};
use crate::error::Result;

/// Default CH9329 baud rate
fn default_ch9329_baud_rate() -> u32 {
    9600
}

/// HID backend type
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
#[derive(Default)]
pub enum HidBackendType {
    /// USB OTG gadget mode
    Otg,
    /// CH9329 serial HID controller
    Ch9329 {
        /// Serial port path
        port: String,
        /// Baud rate (default: 9600)
        #[serde(default = "default_ch9329_baud_rate")]
        baud_rate: u32,
    },
    /// No HID backend (disabled)
    #[default]
    None,
}


impl HidBackendType {
    /// Check if OTG backend is available on this system
    pub fn otg_available() -> bool {
        // Check for USB gadget support
        std::path::Path::new("/sys/class/udc").exists()
    }

    /// Detect the best available backend
    pub fn detect() -> Self {
        // Check for OTG gadget support
        if Self::otg_available() {
            return Self::Otg;
        }

        // Check for common CH9329 serial ports
        let common_ports = [
            "/dev/ttyUSB0",
            "/dev/ttyUSB1",
            "/dev/ttyAMA0",
            "/dev/serial0",
        ];

        for port in &common_ports {
            if std::path::Path::new(port).exists() {
                return Self::Ch9329 {
                    port: port.to_string(),
                    baud_rate: 9600, // Use default baud rate for auto-detection
                };
            }
        }

        Self::None
    }

    /// Get backend name as string
    pub fn name_str(&self) -> &str {
        match self {
            Self::Otg => "otg",
            Self::Ch9329 { .. } => "ch9329",
            Self::None => "none",
        }
    }
}

/// HID backend trait
#[async_trait]
pub trait HidBackend: Send + Sync {
    /// Get backend name
    fn name(&self) -> &'static str;

    /// Initialize the backend
    async fn init(&self) -> Result<()>;

    /// Send a keyboard event
    async fn send_keyboard(&self, event: KeyboardEvent) -> Result<()>;

    /// Send a mouse event
    async fn send_mouse(&self, event: MouseEvent) -> Result<()>;

    /// Send a consumer control event (multimedia keys)
    /// Default implementation returns an error (not supported)
    async fn send_consumer(&self, _event: ConsumerEvent) -> Result<()> {
        Err(crate::error::AppError::BadRequest(
            "Consumer control not supported by this backend".to_string(),
        ))
    }

    /// Reset all inputs (release all keys/buttons)
    async fn reset(&self) -> Result<()>;

    /// Shutdown the backend
    async fn shutdown(&self) -> Result<()>;

    /// Check if backend supports absolute mouse positioning
    fn supports_absolute_mouse(&self) -> bool {
        false
    }

    /// Get screen resolution (for absolute mouse)
    fn screen_resolution(&self) -> Option<(u32, u32)> {
        None
    }

    /// Set screen resolution (for absolute mouse)
    fn set_screen_resolution(&mut self, _width: u32, _height: u32) {}
}

/// HID backend information
#[derive(Debug, Clone, Serialize)]
pub struct HidBackendInfo {
    /// Backend name
    pub name: String,
    /// Backend type
    pub backend_type: String,
    /// Is initialized
    pub initialized: bool,
    /// Supports absolute mouse
    pub absolute_mouse: bool,
    /// Screen resolution (if absolute mouse)
    pub resolution: Option<(u32, u32)>,
}
