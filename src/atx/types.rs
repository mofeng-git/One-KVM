//! ATX data types and structures
//!
//! Defines the configuration and state types for the flexible ATX power control system.
//! Each ATX action (power, reset) can be independently configured with different hardware.

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

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct AtxLedConfig {
    pub enabled: bool,
    pub gpio_chip: String,
    pub gpio_pin: u32,
    pub inverted: bool,
}

impl AtxLedConfig {
    pub fn is_configured(&self) -> bool {
        self.enabled && !self.gpio_chip.is_empty()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AtxState {
    pub available: bool,
    pub power_configured: bool,
    pub reset_configured: bool,
    pub power_status: PowerStatus,
    pub led_supported: bool,
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
    fn test_atx_key_config_default() {
        let config = AtxKeyConfig::default();
        assert_eq!(config.driver, AtxDriverType::None);
        assert!(config.device.is_empty());
        assert_eq!(config.pin, 0);
        assert!(!config.is_configured());
    }

    #[test]
    fn test_atx_key_config_is_configured() {
        let mut config = AtxKeyConfig::default();
        assert!(!config.is_configured());

        config.driver = AtxDriverType::Gpio;
        assert!(!config.is_configured());

        config.device = "/dev/gpiochip0".to_string();
        assert!(config.is_configured());

        config.driver = AtxDriverType::None;
        assert!(!config.is_configured());
    }

    #[test]
    fn test_atx_led_config_default() {
        let config = AtxLedConfig::default();
        assert!(!config.enabled);
        assert!(config.gpio_chip.is_empty());
        assert!(!config.is_configured());
    }

    #[test]
    fn test_atx_led_config_is_configured() {
        let mut config = AtxLedConfig::default();
        assert!(!config.is_configured());

        config.enabled = true;
        assert!(!config.is_configured());

        config.gpio_chip = "/dev/gpiochip0".to_string();
        assert!(config.is_configured());
    }

    #[test]
    fn test_atx_state_default() {
        let state = AtxState::default();
        assert!(!state.available);
        assert!(!state.power_configured);
        assert!(!state.reset_configured);
        assert_eq!(state.power_status, PowerStatus::Unknown);
        assert!(!state.led_supported);
    }
}
