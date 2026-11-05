//! Video processing pipelines.

mod delivery;
mod encoder_state;
mod frame_mailbox;
mod mjpeg_admission;
mod shared;

pub use delivery::EncodedVideoFrameReceiver;
pub(crate) use shared::compact_strided_frame_for_encoder;
pub use shared::{
    EncodedVideoFrame, PipelineAppliedConfig, PipelineLifecycle, PipelineStateNotification,
    SharedVideoPipeline, SharedVideoPipelineConfig, SharedVideoPipelineStats,
};
