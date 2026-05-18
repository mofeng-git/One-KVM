use std::path::Path;
use std::time::Duration;

use crate::error::AppError;
use crate::video::capture::status::signal_status_from_capture_kind;
use crate::video::format::{PixelFormat, Resolution};
use crate::video::signal::SignalStatus;

use super::{BridgeContext, CaptureStream};

pub enum CaptureOpenResult {
    Opened(CaptureStream),
    NoSignal(SignalStatus),
    DeviceLost(String),
    Fatal,
}

pub fn open_capture_stream(
    device_path: &Path,
    resolution: Resolution,
    format: PixelFormat,
    fps: u32,
    buffer_count: u32,
    timeout: Duration,
    bridge_ctx: BridgeContext,
) -> Result<CaptureStream, AppError> {
    CaptureStream::open_with_bridge(
        device_path,
        resolution,
        format,
        fps,
        buffer_count.max(1),
        timeout,
        bridge_ctx,
    )
}

pub fn open_capture_stream_for_retry(
    device_path: &Path,
    resolution: Resolution,
    format: PixelFormat,
    fps: u32,
    buffer_count: u32,
    timeout: Duration,
    bridge_ctx: BridgeContext,
    is_device_lost_message: impl FnOnce(&str) -> bool,
) -> CaptureOpenResult {
    match open_capture_stream(
        device_path,
        resolution,
        format,
        fps,
        buffer_count,
        timeout,
        bridge_ctx,
    ) {
        Ok(stream) => CaptureOpenResult::Opened(stream),
        Err(AppError::CaptureNoSignal { kind }) => {
            CaptureOpenResult::NoSignal(signal_status_from_capture_kind(&kind))
        }
        Err(error) => {
            let reason = error.to_string();
            if is_device_lost_message(&reason) {
                CaptureOpenResult::DeviceLost(reason)
            } else {
                CaptureOpenResult::Fatal
            }
        }
    }
}
