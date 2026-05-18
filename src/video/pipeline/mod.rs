//! Video processing pipelines.

mod encoder_state;
mod shared;

pub use shared::{
    EncodedVideoFrame, PipelineStateNotification, SharedVideoPipeline, SharedVideoPipelineConfig,
    SharedVideoPipelineStats,
};
