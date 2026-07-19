//! USB OTG composite gadget (HID + MSD + Ethernet).

#[cfg(unix)]
pub mod bridge;
#[cfg(unix)]
pub mod configfs;
#[cfg(unix)]
pub mod function;
#[cfg(unix)]
pub mod hid;
#[cfg(unix)]
pub mod manager;
#[cfg(unix)]
pub mod msd;
#[cfg(unix)]
pub mod network;
pub mod report_desc;
pub mod self_check;
#[cfg(unix)]
pub mod service;

#[cfg(unix)]
pub use manager::{wait_for_hid_devices, OtgGadgetManager};
#[cfg(unix)]
pub use msd::{MsdFunction, MsdLunConfig};
#[cfg(unix)]
pub use network::NetworkFunction;
#[cfg(unix)]
pub use service::{HidDevicePaths, OtgNetworkStatus, OtgRuntimeHealth, OtgService};

/// List USB Device Controller names exposed by sysfs.
pub fn list_udc_devices() -> Vec<String> {
    #[cfg(unix)]
    {
        configfs::list_udcs()
    }
    #[cfg(not(unix))]
    {
        Vec::new()
    }
}
