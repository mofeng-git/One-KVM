//! Re-exports of shared video types used by other modules (e.g., webrtc)
//!
//! External modules should import from `crate::video::types` instead of
//! reaching into internal submodules directly.

// From video::format
pub use super::format::{PixelFormat, Resolution};

// From video::frame
pub use super::frame::VideoFrame;

// From video::encoder (codec-level types)
pub use super::encoder::{BitratePreset, VideoCodecType};

// From video::encoder::registry
pub use super::encoder::registry::{EncoderBackend, VideoEncoderType};

// From video::shared_video_pipeline
pub use super::shared_video_pipeline::{
    EncodedVideoFrame, PipelineStateNotification, SharedVideoPipeline, SharedVideoPipelineConfig,
    SharedVideoPipelineStats,
};
