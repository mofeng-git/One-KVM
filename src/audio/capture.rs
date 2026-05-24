#[cfg(all(unix, not(feature = "android")))]
#[path = "capture_linux.rs"]
mod imp;

#[cfg(feature = "android")]
#[path = "capture_android.rs"]
mod imp;

#[cfg(windows)]
#[path = "capture_windows.rs"]
mod imp;

pub use imp::*;
