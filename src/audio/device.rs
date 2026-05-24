#[cfg(all(unix, not(feature = "android")))]
#[path = "device_linux.rs"]
mod imp;

#[cfg(feature = "android")]
#[path = "device_android.rs"]
mod imp;

#[cfg(windows)]
#[path = "device_windows.rs"]
mod imp;

pub use imp::*;
