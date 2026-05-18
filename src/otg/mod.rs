//! USB OTG composite gadget (HID + MSD).

#[cfg(unix)]
pub mod configfs;
pub mod endpoint;
#[cfg(unix)]
pub mod function;
#[cfg(unix)]
pub mod hid;
#[cfg(unix)]
pub mod manager;
#[cfg(unix)]
pub mod msd;
pub mod report_desc;
pub mod self_check;
#[cfg(unix)]
pub mod service;

#[cfg(unix)]
pub use manager::{wait_for_hid_devices, OtgGadgetManager};
#[cfg(unix)]
pub use msd::{MsdFunction, MsdLunConfig};
#[cfg(unix)]
pub use service::{HidDevicePaths, OtgService};

/// List USB Device Controller names exposed by sysfs.
pub fn list_udc_devices() -> Vec<String> {
    let mut devices: Vec<String> = std::fs::read_dir("/sys/class/udc")
        .ok()
        .into_iter()
        .flat_map(|entries| entries.filter_map(|entry| entry.ok()))
        .filter_map(|entry| entry.file_name().to_str().map(str::to_owned))
        .collect();

    devices.sort();
    devices
}
