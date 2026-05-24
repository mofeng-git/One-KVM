//! Platform selection and capability reporting.

#[cfg(feature = "android")]
pub mod android;
#[cfg(feature = "android")]
pub mod android_bionic;
pub mod capabilities;
pub mod defaults;
#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(unix)]
pub mod usb_reset;
#[cfg(windows)]
pub mod windows;

pub use capabilities::{FeatureCapability, PlatformCapabilities, PlatformMode};
