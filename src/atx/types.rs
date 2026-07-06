//! ATX data types and structures
//!
//! Defines the configuration and state types for the ATX power control system.

use serde::{Deserialize, Serialize};
use typeshare::typeshare;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum PowerStatus {
    On,
    Off,
    #[default]
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum HddStatus {
    Active,
    Inactive,
    #[default]
    Unknown,
}

#[typeshare]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum AtxDriverType {
    Gpio,
    UsbRelay,
    Serial,
    #[default]
    None,
}

#[typeshare]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ActiveLevel {
    #[default]
    High,
    Low,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AtxOutputBinding {
    pub enabled: bool,
    pub device: String,
    pub pin: u32,
    pub active_level: ActiveLevel,
}

impl Default for AtxOutputBinding {
    fn default() -> Self {
        Self {
            enabled: false,
            device: String::new(),
            pin: 1,
            active_level: ActiveLevel::High,
        }
    }
}

impl AtxOutputBinding {
    pub fn is_configured_for(&self, driver: AtxDriverType, top_level_device: &str) -> bool {
        if !self.enabled || driver == AtxDriverType::None {
            return false;
        }

        match driver {
            AtxDriverType::Gpio => !self.device.trim().is_empty(),
            AtxDriverType::UsbRelay | AtxDriverType::Serial => !top_level_device.trim().is_empty(),
            AtxDriverType::None => false,
        }
    }
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct AtxInputBinding {
    pub enabled: bool,
    pub device: String,
    pub pin: u32,
    pub active_level: ActiveLevel,
}

impl AtxInputBinding {
    pub fn is_configured(&self) -> bool {
        self.enabled && !self.device.trim().is_empty()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AtxKeyConfig {
    pub driver: AtxDriverType,
    pub device: String,
    pub pin: u32,
    pub active_level: ActiveLevel,
    pub baud_rate: u32,
}

impl Default for AtxKeyConfig {
    fn default() -> Self {
        Self {
            driver: AtxDriverType::None,
            device: String::new(),
            pin: 0,
            active_level: ActiveLevel::High,
            baud_rate: 9600,
        }
    }
}

impl AtxKeyConfig {
    pub fn is_configured(&self) -> bool {
        self.driver != AtxDriverType::None && !self.device.is_empty()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AtxState {
    pub available: bool,
    pub driver: AtxDriverType,
    pub power_configured: bool,
    pub reset_configured: bool,
    pub power_status: PowerStatus,
    pub led_supported: bool,
    pub hdd_status: HddStatus,
    pub hdd_supported: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AtxPowerRequest {
    pub action: AtxAction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AtxAction {
    Short,
    Long,
    Reset,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AtxDevices {
    pub gpio_chips: Vec<String>,
    pub usb_relays: Vec<String>,
    pub serial_ports: Vec<String>,
}

impl Default for AtxDevices {
    fn default() -> Self {
        Self {
            gpio_chips: Vec::new(),
            usb_relays: Vec::new(),
            serial_ports: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_power_status_default() {
        assert_eq!(PowerStatus::default(), PowerStatus::Unknown);
    }

    #[test]
    fn test_atx_driver_type_default() {
        assert_eq!(AtxDriverType::default(), AtxDriverType::None);
    }

    #[test]
    fn test_active_level_default() {
        assert_eq!(ActiveLevel::default(), ActiveLevel::High);
    }

    #[test]
    fn test_atx_output_binding_default() {
        let config = AtxOutputBinding::default();
        assert!(!config.enabled);
        assert!(config.device.is_empty());
        assert_eq!(config.pin, 1);
        assert!(!config.is_configured_for(AtxDriverType::Gpio, ""));
    }

    #[test]
    fn test_atx_output_binding_is_configured() {
        let mut config = AtxOutputBinding::default();
        assert!(!config.is_configured_for(AtxDriverType::Gpio, ""));

        config.enabled = true;
        assert!(!config.is_configured_for(AtxDriverType::Gpio, ""));

        config.device = "/dev/gpiochip0".to_string();
        assert!(config.is_configured_for(AtxDriverType::Gpio, ""));
        assert!(!config.is_configured_for(AtxDriverType::Serial, ""));
        assert!(config.is_configured_for(AtxDriverType::Serial, "/dev/ttyUSB0"));
    }

    #[test]
    fn test_atx_key_config_default() {
        let config = AtxKeyConfig::default();
        assert_eq!(config.driver, AtxDriverType::None);
        assert!(config.device.is_empty());
        assert_eq!(config.pin, 0);
        assert!(!config.is_configured());
    }

    #[test]
    fn test_atx_input_binding_default() {
        let config = AtxInputBinding::default();
        assert!(!config.enabled);
        assert!(config.device.is_empty());
        assert!(!config.is_configured());
    }

    #[test]
    fn test_atx_input_binding_is_configured() {
        let mut config = AtxInputBinding::default();
        assert!(!config.is_configured());

        config.enabled = true;
        assert!(!config.is_configured());

        config.device = "/dev/gpiochip0".to_string();
        assert!(config.is_configured());
    }

    #[test]
    fn test_atx_state_default() {
        let state = AtxState::default();
        assert!(!state.available);
        assert_eq!(state.driver, AtxDriverType::None);
        assert!(!state.power_configured);
        assert!(!state.reset_configured);
        assert_eq!(state.power_status, PowerStatus::Unknown);
        assert!(!state.led_supported);
        assert_eq!(state.hdd_status, HddStatus::Unknown);
        assert!(!state.hdd_supported);
    }
}
