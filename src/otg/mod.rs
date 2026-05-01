//! USB OTG composite gadget (HID + MSD).

pub mod configfs;
pub mod endpoint;
pub mod function;
pub mod hid;
pub mod manager;
pub mod msd;
pub mod report_desc;
pub mod service;

pub use manager::{wait_for_hid_devices, OtgGadgetManager};
pub use msd::{MsdFunction, MsdLunConfig};
pub use service::{HidDevicePaths, OtgService};
