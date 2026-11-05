//! Video capture implementations and capture-state helpers.

use std::fmt;
use std::io;

pub(crate) mod runtime;
pub(crate) mod status;

pub const DEFAULT_CAPTURE_BUFFER_COUNT: u32 = 4;

/// Expected source changes are control flow, not stringly typed I/O errors.
#[derive(Debug)]
pub enum CaptureReadError {
    SourceChanged,
    Io(io::Error),
}

impl CaptureReadError {
    pub fn as_io_error(&self) -> Option<&io::Error> {
        match self {
            Self::SourceChanged => None,
            Self::Io(error) => Some(error),
        }
    }
}

impl From<io::Error> for CaptureReadError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl fmt::Display for CaptureReadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SourceChanged => formatter.write_str("capture source changed"),
            Self::Io(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for CaptureReadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.as_io_error()
            .map(|error| error as &(dyn std::error::Error + 'static))
    }
}

#[cfg(unix)]
mod linux;
#[cfg(windows)]
#[path = "windows.rs"]
mod linux;

pub use linux::*;
