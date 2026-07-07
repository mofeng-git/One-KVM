//! Video capture implementations and capture-state helpers.

pub(crate) mod runtime;
pub(crate) mod status;

pub const DEFAULT_CAPTURE_BUFFER_COUNT: u32 = 4;

#[cfg(unix)]
mod linux;
#[cfg(windows)]
#[path = "windows.rs"]
mod linux;

pub use linux::*;
