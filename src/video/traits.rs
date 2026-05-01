//! Traits for video output consumers (WebRTC, RTSP, RustDesk, etc.)

use std::path::PathBuf;
use std::sync::Arc;

use super::types::{
    BitratePreset, PixelFormat, Resolution, SharedVideoPipeline, SharedVideoPipelineConfig,
    SharedVideoPipelineStats, VideoCodecType,
};
use crate::error::Result;
use crate::events::EventBus;
use crate::hid::HidController;

/// Trait for video output consumers that receive encoded video frames.
///
/// Implemented by `WebRtcStreamer`. `VideoStreamManager` depends on this
/// trait instead of the concrete type, breaking the video <-> webrtc
/// circular import.
#[async_trait::async_trait]
pub trait VideoOutput: Send + Sync {
    async fn set_event_bus(&self, events: Arc<EventBus>);
    async fn update_video_config(&self, resolution: Resolution, format: PixelFormat, fps: u32);
    async fn set_capture_device(
        &self,
        device_path: PathBuf,
        jpeg_quality: u8,
        subdev_path: Option<PathBuf>,
        bridge_kind: Option<String>,
        v4l2_driver: Option<String>,
    );
    async fn current_video_codec(&self) -> VideoCodecType;
    async fn is_hardware_encoding(&self) -> bool;
    async fn close_all_sessions(&self);
    async fn close_all_sessions_and_release_device(&self) -> usize;
    async fn session_count(&self) -> usize;
    async fn set_hid_controller(&self, hid: Arc<HidController>);
    async fn set_audio_enabled(&self, enabled: bool) -> Result<()>;
    async fn is_audio_enabled(&self) -> bool;
    async fn reconnect_audio_sources(&self);
    async fn ensure_video_pipeline_for_external(&self) -> Result<Arc<SharedVideoPipeline>>;
    async fn get_pipeline_config(&self) -> Option<SharedVideoPipelineConfig>;
    async fn set_video_codec(&self, codec: VideoCodecType) -> Result<()>;
    async fn set_bitrate_preset(&self, preset: BitratePreset) -> Result<()>;
    async fn request_keyframe(&self) -> Result<()>;
    async fn current_video_geometry(&self) -> (Resolution, PixelFormat, u32);
    async fn pipeline_stats(&self) -> Option<SharedVideoPipelineStats>;
}
