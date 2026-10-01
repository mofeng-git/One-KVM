//! Video processing pipelines.

mod delivery;
mod encoder_state;
mod frame_mailbox;
mod shared;

pub use delivery::EncodedVideoFrameReceiver;
pub use shared::{
    EncodedVideoFrame, PipelineAppliedConfig, PipelineLifecycle, PipelineStateNotification,
    SharedVideoPipeline, SharedVideoPipelineConfig, SharedVideoPipelineStats,
};
