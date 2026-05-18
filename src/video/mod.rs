//! Video capture and streaming module
//!
//! This module provides V4L2 video capture, encoding, and streaming functionality.

pub mod capture;
pub mod codec;
pub mod codec_constraints;
pub mod device;
pub mod format;
pub mod frame;
pub mod pipeline;
pub mod signal;
pub mod stream_manager;
pub mod streamer;
pub mod traits;
pub mod types;

pub use codec::{H264Encoder, H264EncoderType, JpegEncoder, PixelConverter, Yuv420pBuffer};
pub use device::{VideoDevice, VideoDeviceInfo};
pub use format::PixelFormat;
pub use frame::VideoFrame;
pub use pipeline::{
    EncodedVideoFrame, SharedVideoPipeline, SharedVideoPipelineConfig, SharedVideoPipelineStats,
};
pub use signal::SignalStatus;
pub use stream_manager::VideoStreamManager;
pub use streamer::{Streamer, StreamerState};

impl From<SignalStatus> for streamer::StreamerState {
    fn from(value: SignalStatus) -> Self {
        match value {
            SignalStatus::NoCable => streamer::StreamerState::NoCable,
            SignalStatus::NoSync => streamer::StreamerState::NoSync,
            SignalStatus::OutOfRange => streamer::StreamerState::OutOfRange,
            SignalStatus::NoSignal => streamer::StreamerState::NoSignal,
            SignalStatus::UvcUsbError => streamer::StreamerState::UvcUsbError,
            SignalStatus::UvcCaptureStall => streamer::StreamerState::UvcCaptureStall,
        }
    }
}
