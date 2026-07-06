use serde::{Deserialize, Serialize};
use typeshare::typeshare;

pub use crate::atx::{ActiveLevel, AtxDriverType, AtxInputBinding, AtxOutputBinding};

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AtxConfig {
    pub enabled: bool,
    pub driver: AtxDriverType,
    pub device: String,
    pub baud_rate: u32,
    pub power: AtxOutputBinding,
    pub reset: AtxOutputBinding,
    pub led: AtxInputBinding,
    pub hdd: AtxInputBinding,
    pub wol_interface: String,
}

impl Default for AtxConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            driver: AtxDriverType::None,
            device: String::new(),
            baud_rate: 9600,
            power: AtxOutputBinding::default(),
            reset: AtxOutputBinding::default(),
            led: AtxInputBinding::default(),
            hdd: AtxInputBinding::default(),
            wol_interface: String::new(),
        }
    }
}

impl AtxConfig {
    pub fn normalize(&mut self) {
        if self.driver == AtxDriverType::None {
            self.enabled = false;
        }

        if self.driver != AtxDriverType::Gpio {
            self.led.enabled = false;
            self.hdd.enabled = false;
        }
    }

    pub fn to_controller_config(&self) -> crate::atx::AtxControllerConfig {
        crate::atx::AtxControllerConfig {
            enabled: self.enabled,
            driver: self.driver,
            device: self.device.clone(),
            baud_rate: self.baud_rate,
            power: self.power.clone(),
            reset: self.reset.clone(),
            led: self.led.clone(),
            hdd: self.hdd.clone(),
        }
    }
}
