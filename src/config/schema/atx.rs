use serde::{Deserialize, Serialize};
use typeshare::typeshare;

pub use crate::atx::{ActiveLevel, AtxDriverType, AtxKeyConfig, AtxLedConfig};

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
#[derive(Default)]
pub struct AtxConfig {
    pub enabled: bool,
    pub power: AtxKeyConfig,
    pub reset: AtxKeyConfig,
    pub led: AtxLedConfig,
    pub wol_interface: String,
}

impl AtxConfig {
    pub fn to_controller_config(&self) -> crate::atx::AtxControllerConfig {
        crate::atx::AtxControllerConfig {
            enabled: self.enabled,
            power: self.power.clone(),
            reset: self.reset.clone(),
            led: self.led.clone(),
        }
    }
}

