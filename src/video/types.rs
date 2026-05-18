//! Re-exports of shared video types used by other modules (e.g., webrtc)
//!
//! External modules should import from `crate::video::types` instead of
//! reaching into internal submodules directly.

// From video::format
pub use super::format::{PixelFormat, Resolution};

// From video::frame
pub use super::frame::VideoFrame;

// From video::codec (codec-level types)
pub use super::codec::{BitratePreset, VideoCodecType};

// From video::codec::registry
pub use super::codec::registry::{EncoderBackend, VideoEncoderType};

// From video::pipeline
pub use super::pipeline::{
    EncodedVideoFrame, PipelineStateNotification, SharedVideoPipeline, SharedVideoPipelineConfig,
    SharedVideoPipelineStats,
};
