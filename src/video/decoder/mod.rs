//! Video decoder implementations
//!
//! This module provides video decoding capabilities including:
//! - MJPEG VAAPI hardware decoding (outputs NV12)
//! - MJPEG turbojpeg decoding (outputs YUV420P directly)

pub mod mjpeg;

pub use mjpeg::{
    DecodedYuv420pFrame, MjpegTurboDecoder, MjpegVaapiDecoder, MjpegVaapiDecoderConfig,
};
