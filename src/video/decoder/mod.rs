//! Video decoder implementations
//!
//! This module provides video decoding capabilities.

#[cfg(any(target_arch = "aarch64", target_arch = "arm"))]
pub mod mjpeg_rkmpp;
pub mod mjpeg_turbo;

#[cfg(any(target_arch = "aarch64", target_arch = "arm"))]
pub use mjpeg_rkmpp::MjpegRkmppDecoder;
pub use mjpeg_turbo::MjpegTurboDecoder;
