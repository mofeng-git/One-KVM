#[cfg(unix)]
#[path = "capture_linux.rs"]
mod imp;

#[cfg(windows)]
#[path = "capture_windows.rs"]
mod imp;

pub use imp::*;
