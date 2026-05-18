//! Platform selection and capability reporting.

pub mod capabilities;
pub mod defaults;
pub mod linux;
#[cfg(unix)]
pub mod usb_reset;
pub mod windows;

pub use capabilities::{FeatureCapability, PlatformCapabilities, PlatformMode};
