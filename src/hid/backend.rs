//! `HidBackend` trait plus serde `HidBackendType` (OTG | CH9329 | disabled).

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::watch;

use super::types::{ConsumerEvent, KeyboardEvent, MouseEvent};
use crate::error::Result;
use crate::events::LedState;

fn default_ch9329_baud_rate() -> u32 {
    9600
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
#[derive(Default)]
pub enum HidBackendType {
    Otg,
    Ch9329 {
        port: String,
        #[serde(default = "default_ch9329_baud_rate")]
        baud_rate: u32,
    },
    #[default]
    None,
}

impl HidBackendType {
    pub fn name_str(&self) -> &str {
        match self {
            Self::Otg => "otg",
            Self::Ch9329 { .. } => "ch9329",
            Self::None => "none",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HidBackendRuntimeSnapshot {
    pub initialized: bool,
    pub online: bool,
    pub supports_absolute_mouse: bool,
    pub keyboard_leds_enabled: bool,
    pub led_state: LedState,
    pub screen_resolution: Option<(u32, u32)>,
    pub device: Option<String>,
    pub error: Option<String>,
    pub error_code: Option<String>,
}

#[async_trait]
pub trait HidBackend: Send + Sync {
    async fn init(&self) -> Result<()>;

    async fn send_keyboard(&self, event: KeyboardEvent) -> Result<()>;

    async fn send_mouse(&self, event: MouseEvent) -> Result<()>;

    async fn send_consumer(&self, _event: ConsumerEvent) -> Result<()> {
        Err(crate::error::AppError::BadRequest(
            "Consumer control not supported by this backend".to_string(),
        ))
    }

    async fn reset(&self) -> Result<()>;

    async fn shutdown(&self) -> Result<()>;

    fn runtime_snapshot(&self) -> HidBackendRuntimeSnapshot;

    fn subscribe_runtime(&self) -> watch::Receiver<()>;

    fn set_screen_resolution(&self, _width: u32, _height: u32) {}
}
