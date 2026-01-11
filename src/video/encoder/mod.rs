//! Video encoder implementations
//!
//! This module provides video encoding capabilities including:
//! - JPEG encoding for raw frames (YUYV, NV12, etc.)
//! - H264 encoding (hardware + software)
//! - H265 encoding (hardware only)
//! - VP8 encoding (hardware only - VAAPI)
//! - VP9 encoding (hardware only - VAAPI)
//! - WebRTC video codec abstraction
//! - Encoder registry for automatic detection

pub mod codec;
pub mod h264;
pub mod h265;
pub mod jpeg;
pub mod registry;
pub mod traits;
pub mod vp8;
pub mod vp9;

// Core traits and types
pub use traits::{
    BitratePreset, EncodedFormat, EncodedFrame, Encoder, EncoderConfig, EncoderFactory,
};

// WebRTC codec abstraction
pub use codec::{CodecFrame, VideoCodec, VideoCodecConfig, VideoCodecFactory, VideoCodecType};

// Encoder registry
pub use registry::{AvailableEncoder, EncoderBackend, EncoderRegistry, VideoEncoderType};

// H264 encoder
pub use h264::{H264Config, H264Encoder, H264EncoderType, H264InputFormat};

// H265 encoder (hardware only)
pub use h265::{H265Config, H265Encoder, H265EncoderType, H265InputFormat};

// VP8 encoder (hardware only)
pub use vp8::{VP8Config, VP8Encoder, VP8EncoderType, VP8InputFormat};

// VP9 encoder (hardware only)
pub use vp9::{VP9Config, VP9Encoder, VP9EncoderType, VP9InputFormat};

// JPEG encoder
pub use jpeg::JpegEncoder;
