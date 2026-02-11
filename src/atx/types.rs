//! ATX data types and structures
//!
//! Defines the configuration and state types for the flexible ATX power control system.
//! Each ATX action (power, reset) can be independently configured with different hardware.

use serde::{Deserialize, Serialize};
use typeshare::typeshare;

/// Power status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PowerStatus {
    /// Power is on
    On,
    /// Power is off
    Off,
    /// Power status unknown (no LED connected)
    Unknown,
}

impl Default for PowerStatus {
    fn default() -> Self {
        Self::Unknown
    }
}

/// Driver type for ATX key operations
#[typeshare]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AtxDriverType {
    /// GPIO control via Linux character device
    Gpio,
    /// USB HID relay module
    UsbRelay,
    /// Serial/COM port relay (LCUS type)
    Serial,
    /// Disabled / Not configured
    None,
}

impl Default for AtxDriverType {
    fn default() -> Self {
        Self::None
    }
}

/// Active level for GPIO pins
#[typeshare]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ActiveLevel {
    /// Active high (default for most cases)
    High,
    /// Active low (inverted)
    Low,
}

impl Default for ActiveLevel {
    fn default() -> Self {
        Self::High
    }
}

/// Configuration for a single ATX key (power or reset)
/// This is the "four-tuple" configuration: (driver, device, pin/channel, level)
#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AtxKeyConfig {
    /// Driver type (GPIO or USB Relay)
    pub driver: AtxDriverType,
    /// Device path:
    /// - For GPIO: /dev/gpiochipX
    /// - For USB Relay: /dev/hidrawX
    pub device: String,
    /// Pin or channel number:
    /// - For GPIO: GPIO pin number
    /// - For USB Relay: relay channel (0-based)
    pub pin: u32,
    /// Active level (only applicable to GPIO, ignored for USB Relay)
    pub active_level: ActiveLevel,
    /// Baud rate for serial relay (start with 9600)
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
    /// Check if this key is configured
    pub fn is_configured(&self) -> bool {
        self.driver != AtxDriverType::None && !self.device.is_empty()
    }
}

/// LED sensing configuration (optional)
#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AtxLedConfig {
    /// Whether LED sensing is enabled
    pub enabled: bool,
    /// GPIO chip for LED sensing
    pub gpio_chip: String,
    /// GPIO pin for LED input
    pub gpio_pin: u32,
    /// Whether LED is active low (inverted logic)
    pub inverted: bool,
}

impl Default for AtxLedConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            gpio_chip: String::new(),
            gpio_pin: 0,
            inverted: false,
        }
    }
}

impl AtxLedConfig {
    /// Check if LED sensing is configured
    pub fn is_configured(&self) -> bool {
        self.enabled && !self.gpio_chip.is_empty()
    }
}

/// ATX state information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AtxState {
    /// Whether ATX feature is available/enabled
    pub available: bool,
    /// Whether power button is configured
    pub power_configured: bool,
    /// Whether reset button is configured
    pub reset_configured: bool,
    /// Current power status
    pub power_status: PowerStatus,
    /// Whether power LED sensing is supported
    pub led_supported: bool,
}

impl Default for AtxState {
    fn default() -> Self {
        Self {
            available: false,
            power_configured: false,
            reset_configured: false,
            power_status: PowerStatus::Unknown,
            led_supported: false,
        }
    }
}

/// ATX power action request
#[derive(Debug, Clone, Deserialize)]
pub struct AtxPowerRequest {
    /// Action to perform: "short", "long", "reset"
    pub action: AtxAction,
}

/// ATX power action
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AtxAction {
    /// Short press power button (turn on or graceful shutdown)
    Short,
    /// Long press power button (force power off)
    Long,
    /// Press reset button
    Reset,
}

/// Available ATX devices for discovery
#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AtxDevices {
    /// Available GPIO chips (/dev/gpiochip*)
    pub gpio_chips: Vec<String>,
    /// Available USB HID relay devices (/dev/hidraw*)
    pub usb_relays: Vec<String>,
    /// Available Serial ports (/dev/ttyUSB*)
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
        assert!(!config.is_configured()); // device still empty

        config.device = "/dev/gpiochip0".to_string();
        assert!(config.is_configured());

        config.driver = AtxDriverType::None;
        assert!(!config.is_configured()); // driver is None
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
        assert!(!config.is_configured()); // gpio_chip still empty

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
    }
}
