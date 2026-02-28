//! Video capture and streaming module
//!
//! This module provides V4L2 video capture, encoding, and streaming functionality.

pub mod capture;
pub mod codec_constraints;
pub mod convert;
pub mod decoder;
pub mod device;
pub mod encoder;
pub mod format;
pub mod frame;
pub mod h264_pipeline;
pub mod shared_video_pipeline;
pub mod stream_manager;
pub mod streamer;
pub mod video_session;

pub use capture::VideoCapturer;
pub use convert::{PixelConverter, Yuv420pBuffer};
pub use device::{VideoDevice, VideoDeviceInfo};
pub use encoder::{H264Encoder, H264EncoderType, JpegEncoder};
pub use format::PixelFormat;
pub use frame::VideoFrame;
pub use h264_pipeline::{H264Pipeline, H264PipelineBuilder, H264PipelineConfig};
pub use shared_video_pipeline::{
    EncodedVideoFrame, SharedVideoPipeline, SharedVideoPipelineConfig, SharedVideoPipelineStats,
};
pub use stream_manager::VideoStreamManager;
pub use streamer::{Streamer, StreamerState};
pub use video_session::{
    CodecInfo, VideoSessionInfo, VideoSessionManager, VideoSessionManagerConfig, VideoSessionState,
};
