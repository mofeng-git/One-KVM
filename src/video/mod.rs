//! Video capture and streaming module
//!
//! This module provides V4L2 video capture, encoding, and streaming functionality.

pub mod codec_constraints;
pub mod convert;
pub mod csi_bridge;
pub mod decoder;
pub mod device;
pub mod encoder;
pub mod format;
pub mod frame;
pub mod shared_video_pipeline;
pub mod stream_manager;
pub mod streamer;
pub mod v4l2r_capture;

pub use convert::{PixelConverter, Yuv420pBuffer};
pub use device::{VideoDevice, VideoDeviceInfo};
pub use encoder::{H264Encoder, H264EncoderType, JpegEncoder};
pub use format::PixelFormat;
pub use frame::VideoFrame;
pub use shared_video_pipeline::{
    EncodedVideoFrame, SharedVideoPipeline, SharedVideoPipelineConfig, SharedVideoPipelineStats,
};
pub use stream_manager::VideoStreamManager;
pub use streamer::{Streamer, StreamerState};

/// Fine-grained signal status reported by CSI/HDMI bridge devices.
///
/// Only `rk_hdmirx` / `rkcif` / tc358743-class bridges can distinguish these
/// via `VIDIOC_QUERY_DV_TIMINGS` errno; USB UVC devices always report `Ok`
/// until they fail with a generic timeout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalStatus {
    /// HDMI cable physically disconnected (`ENOLINK`).
    NoCable,
    /// TMDS signal present but timings cannot be locked (`ENOLCK`).
    NoSync,
    /// Timings outside of hardware capability (`ERANGE`).
    OutOfRange,
    /// Generic "no usable source" (fallback for EINVAL / EIO / unknown errnos).
    NoSignal,
}

impl SignalStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            SignalStatus::NoCable => "no_cable",
            SignalStatus::NoSync => "no_sync",
            SignalStatus::OutOfRange => "out_of_range",
            SignalStatus::NoSignal => "no_signal",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        Some(match s {
            "no_cable" => SignalStatus::NoCable,
            "no_sync" => SignalStatus::NoSync,
            "out_of_range" => SignalStatus::OutOfRange,
            "no_signal" => SignalStatus::NoSignal,
            _ => return None,
        })
    }
}

impl From<SignalStatus> for streamer::StreamerState {
    fn from(value: SignalStatus) -> Self {
        match value {
            SignalStatus::NoCable => streamer::StreamerState::NoCable,
            SignalStatus::NoSync => streamer::StreamerState::NoSync,
            SignalStatus::OutOfRange => streamer::StreamerState::OutOfRange,
            SignalStatus::NoSignal => streamer::StreamerState::NoSignal,
        }
    }
}

pub(crate) fn is_rk_hdmirx_driver(driver: &str, card: &str) -> bool {
    driver.eq_ignore_ascii_case("rk_hdmirx") || card.eq_ignore_ascii_case("rk_hdmirx")
}

pub(crate) fn is_rk_hdmirx_device(device: &device::VideoDeviceInfo) -> bool {
    is_rk_hdmirx_driver(&device.driver, &device.card)
}

pub(crate) fn is_rkcif_driver(driver: &str) -> bool {
    driver.eq_ignore_ascii_case("rkcif")
}

/// Unified check for CSI/HDMI bridge devices (rk_hdmirx, rkcif, etc.)
/// that require special enumeration and format-selection logic.
pub(crate) fn is_csi_hdmi_bridge(device: &device::VideoDeviceInfo) -> bool {
    is_rk_hdmirx_device(device) || is_rkcif_driver(&device.driver)
}
