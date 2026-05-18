//! Video capture implementations and capture-state helpers.

pub(crate) mod runtime;
pub(crate) mod status;

#[cfg(unix)]
mod linux;
#[cfg(windows)]
#[path = "windows.rs"]
mod linux;

pub use linux::*;
