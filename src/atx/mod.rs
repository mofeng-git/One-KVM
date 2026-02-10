//! ATX Power Control Module
//!
//! Provides ATX power management functionality for IP-KVM.
//! Supports flexible hardware binding with independent configuration for each action.
//!
//! # Features
//!
//! - Power button control (short press for on/graceful shutdown, long press for force off)
//! - Reset button control
//! - Power status monitoring via LED sensing (GPIO only)
//! - Independent hardware binding for each action (GPIO or USB relay)
//! - Hot-reload configuration support
//!
//! # Hardware Support
//!
//! - **GPIO**: Uses Linux GPIO character device (/dev/gpiochipX) for direct hardware control
//! - **USB Relay**: Uses HID USB relay modules for isolated switching
//!
//! # Example
//!
//! ```ignore
//! use one_kvm::atx::{AtxController, AtxControllerConfig, AtxKeyConfig, AtxDriverType, ActiveLevel};
//!
//! let config = AtxControllerConfig {
//!     enabled: true,
//!     power: AtxKeyConfig {
//!         driver: AtxDriverType::Gpio,
//!         device: "/dev/gpiochip0".to_string(),
//!         pin: 5,
//!         active_level: ActiveLevel::High,
//!     },
//!     reset: AtxKeyConfig {
//!         driver: AtxDriverType::UsbRelay,
//!         device: "/dev/hidraw0".to_string(),
//!         pin: 0,
//!         active_level: ActiveLevel::High,
//!     },
//!     led: Default::default(),
//! };
//!
//! let controller = AtxController::new(config);
//! controller.init().await?;
//! controller.power_short().await?;  // Turn on or graceful shutdown
//! ```

mod controller;
mod executor;
mod led;
mod types;
mod wol;

pub use controller::{AtxController, AtxControllerConfig};
pub use executor::timing;
pub use types::{
    ActiveLevel, AtxAction, AtxDevices, AtxDriverType, AtxKeyConfig, AtxLedConfig, AtxPowerRequest,
    AtxState, PowerStatus,
};
pub use wol::send_wol;

/// Discover available ATX devices on the system
///
/// Scans for GPIO chips and USB HID relay devices in a single pass.
pub fn discover_devices() -> AtxDevices {
    let mut devices = AtxDevices::default();

    // Single pass through /dev directory
    if let Ok(entries) = std::fs::read_dir("/dev") {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.starts_with("gpiochip") {
                devices.gpio_chips.push(format!("/dev/{}", name_str));
            } else if name_str.starts_with("hidraw") {
                devices.usb_relays.push(format!("/dev/{}", name_str));
            }
        }
    }

    devices.gpio_chips.sort();
    devices.usb_relays.sort();

    devices
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_discover_devices() {
        let _devices = discover_devices();
    }

    #[test]
    fn test_module_exports() {
        // Verify all public exports are accessible
        let _: AtxDriverType = AtxDriverType::None;
        let _: ActiveLevel = ActiveLevel::High;
        let _: AtxKeyConfig = AtxKeyConfig::default();
        let _: AtxLedConfig = AtxLedConfig::default();
        let _: AtxState = AtxState::default();
        let _: AtxDevices = AtxDevices::default();
    }
}
