//! ATX Power Control Module
//!
//! Provides ATX power management functionality for IP-KVM.
//! Supports flexible hardware binding with independent configuration for each action.

mod controller;
#[cfg(not(unix))]
mod disabled_key;
mod executor;
#[cfg(unix)]
mod gpio_linux;
#[cfg(unix)]
mod hidraw_linux;
#[cfg(unix)]
mod led;
#[cfg(not(unix))]
#[path = "disabled_led.rs"]
mod led;
mod serial_relay;
mod traits;
mod types;
mod wol;

pub use controller::{AtxController, AtxControllerConfig};
pub use executor::timing;
pub use types::{
    ActiveLevel, AtxAction, AtxDevices, AtxDriverType, AtxKeyConfig, AtxLedConfig, AtxPowerRequest,
    AtxState, PowerStatus,
};
pub use wol::{list_wol_history, record_wol_history, send_wol};

#[cfg(any(unix, test))]
fn hidraw_uevent_is_usb_relay(uevent: &str) -> bool {
    let upper = uevent.to_ascii_uppercase();
    upper.contains("000016C0:000005DF")
        || upper.contains("00005131:00002007")
        || upper.contains("16C0:05DF")
        || upper.contains("5131:2007")
        || upper.contains("PRODUCT=16C0/5DF")
        || upper.contains("PRODUCT=5131/2007")
        || upper.contains("USBRELAY")
        || upper.contains("USB RELAY")
}

#[cfg(unix)]
fn is_usb_relay_hidraw(name: &str) -> bool {
    let uevent_path = format!("/sys/class/hidraw/{}/device/uevent", name);
    std::fs::read_to_string(uevent_path)
        .map(|uevent| hidraw_uevent_is_usb_relay(&uevent))
        .unwrap_or(false)
}

/// Discover available ATX devices on the system
///
/// Scans for GPIO chips, LCUS USB HID relay devices, and serial relay ports.
pub fn discover_devices() -> AtxDevices {
    let mut devices = AtxDevices::default();

    devices.serial_ports = crate::utils::list_serial_ports();

    #[cfg(unix)]
    if let Ok(entries) = std::fs::read_dir("/dev") {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.starts_with("gpiochip") {
                devices.gpio_chips.push(format!("/dev/{}", name_str));
            }
            #[cfg(unix)]
            if name_str.starts_with("hidraw") && is_usb_relay_hidraw(&name_str) {
                devices.usb_relays.push(format!("/dev/{}", name_str));
            }
            if name_str.starts_with("ttyUSB") || name_str.starts_with("ttyACM") {
                devices.serial_ports.push(format!("/dev/{}", name_str));
            }
        }
    }

    devices.gpio_chips.sort();
    devices.usb_relays.sort();
    devices.serial_ports.sort();
    devices.serial_ports.dedup();

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
    fn test_hidraw_uevent_detects_usb_relay_id() {
        assert!(hidraw_uevent_is_usb_relay(
            "HID_ID=0003:000016C0:000005DF\nHID_NAME=www.dcttech.com USBRelay2\n"
        ));
    }

    #[test]
    fn test_hidraw_uevent_detects_5131_usb_relay_id() {
        assert!(hidraw_uevent_is_usb_relay(
            "HID_ID=0003:00005131:00002007\n"
        ));
        assert!(hidraw_uevent_is_usb_relay("PRODUCT=5131/2007/100"));
    }

    #[test]
    fn test_hidraw_uevent_rejects_unrelated_hid() {
        assert!(!hidraw_uevent_is_usb_relay(
            "HID_ID=0003:0000046D:0000C534\nHID_NAME=Logitech USB Receiver\n"
        ));
    }

    #[test]
    fn test_module_exports() {
        let _: AtxDriverType = AtxDriverType::None;
        let _: ActiveLevel = ActiveLevel::High;
        let _: AtxKeyConfig = AtxKeyConfig::default();
        let _: AtxLedConfig = AtxLedConfig::default();
        let _: AtxState = AtxState::default();
        let _: AtxDevices = AtxDevices::default();
    }
}
