#[cfg(unix)]
#[path = "device_linux.rs"]
mod imp;

#[cfg(windows)]
#[path = "device_windows.rs"]
mod imp;

pub use imp::*;
