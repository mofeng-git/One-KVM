use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::error::{AppError, Result};
use crate::video::device::bridge::{CsiBridgeKind, ProbeResult};
use crate::video::device::{directshow_display_name_from_path, normalize_windows_device_path};
use crate::video::format::{PixelFormat, Resolution};

pub const SOURCE_CHANGED_MARKER: &str = "dshow_source_changed";

pub fn is_source_changed_error(err: &io::Error) -> bool {
    err.get_ref()
        .map(|inner| inner.to_string() == SOURCE_CHANGED_MARKER)
        .unwrap_or(false)
}

#[derive(Debug, Clone, Copy)]
pub struct CaptureMeta {
    pub bytes_used: usize,
    pub sequence: u64,
}

#[derive(Debug, Clone, Default)]
pub struct BridgeContext {
    pub subdev_path: Option<PathBuf>,
    pub kind: Option<CsiBridgeKind>,
}

impl BridgeContext {
    pub fn from_parts(subdev_path: Option<PathBuf>, kind: Option<CsiBridgeKind>) -> Self {
        Self { subdev_path, kind }
    }

    pub fn has_subdev(&self) -> bool {
        false
    }
}

pub struct CaptureStream {
    capture: hwcodec::capture::DshowCapture,
    resolution: Resolution,
    format: PixelFormat,
    stride: u32,
}

unsafe impl Send for CaptureStream {}

impl CaptureStream {
    pub fn open(
        device_path: impl AsRef<Path>,
        resolution: Resolution,
        format: PixelFormat,
        fps: u32,
        buffer_count: u32,
        timeout: Duration,
    ) -> Result<Self> {
        let _ = buffer_count;
        let path = normalize_windows_device_path(device_path);
        let display_name = directshow_display_name_from_path(&path).ok_or_else(|| {
            AppError::VideoError(format!(
                "Unsupported DirectShow device path: {}",
                path.display()
            ))
        })?;
        let capture = hwcodec::capture::DshowCapture::open(
            &display_name,
            resolution.width as i32,
            resolution.height as i32,
            fps as i32,
            map_pixel_format(format),
            timeout.as_millis().clamp(1, i32::MAX as u128) as i32,
        )
        .map_err(|e| AppError::VideoError(format!("Failed to open DirectShow capture: {}", e)))?;
        let info = capture.info().map_err(|e| {
            AppError::VideoError(format!("Failed to query DirectShow capture: {}", e))
        })?;
        let actual_format = map_capture_format(info.pixel_format)?;
        let actual_resolution =
            Resolution::new(info.width.max(1) as u32, info.height.max(1) as u32);

        Ok(Self {
            capture,
            resolution: actual_resolution,
            format: actual_format,
            stride: info.stride.max(0) as u32,
        })
    }

    pub fn open_with_bridge(
        device_path: impl AsRef<Path>,
        resolution: Resolution,
        format: PixelFormat,
        fps: u32,
        buffer_count: u32,
        timeout: Duration,
        bridge: BridgeContext,
    ) -> Result<Self> {
        let _ = bridge;
        Self::open(device_path, resolution, format, fps, buffer_count, timeout)
    }

    pub fn resolution(&self) -> Resolution {
        self.resolution
    }

    pub fn format(&self) -> PixelFormat {
        self.format
    }

    pub fn stride(&self) -> u32 {
        self.stride
    }

    pub fn next_into(&mut self, dst: &mut Vec<u8>) -> io::Result<CaptureMeta> {
        match self.capture.read_packet() {
            Ok((packet, sequence)) => {
                dst.clear();
                dst.extend_from_slice(&packet);
                Ok(CaptureMeta {
                    bytes_used: packet.len(),
                    sequence,
                })
            }
            Err(err) => {
                let kind = if err.code == -110 {
                    io::ErrorKind::TimedOut
                } else {
                    io::ErrorKind::Other
                };
                Err(io::Error::new(kind, err.message))
            }
        }
    }

    pub fn probe_bridge_signal_with_timeout(&self, _limit: Duration) -> Option<ProbeResult> {
        None
    }
}

fn map_pixel_format(format: PixelFormat) -> hwcodec::capture::CapturePixelFormat {
    match format {
        PixelFormat::Mjpeg => hwcodec::capture::CapturePixelFormat::Mjpeg,
        PixelFormat::Jpeg => hwcodec::capture::CapturePixelFormat::Jpeg,
        PixelFormat::Yuyv => hwcodec::capture::CapturePixelFormat::Yuyv,
        PixelFormat::Yvyu => hwcodec::capture::CapturePixelFormat::Yvyu,
        PixelFormat::Uyvy => hwcodec::capture::CapturePixelFormat::Uyvy,
        PixelFormat::Nv12 => hwcodec::capture::CapturePixelFormat::Nv12,
        PixelFormat::Nv21 => hwcodec::capture::CapturePixelFormat::Nv21,
        PixelFormat::Nv16 => hwcodec::capture::CapturePixelFormat::Nv16,
        PixelFormat::Nv24 => hwcodec::capture::CapturePixelFormat::Nv24,
        PixelFormat::Yuv420 => hwcodec::capture::CapturePixelFormat::Yuv420,
        PixelFormat::Yvu420 => hwcodec::capture::CapturePixelFormat::Yvu420,
        PixelFormat::Rgb24 => hwcodec::capture::CapturePixelFormat::Rgb24,
        PixelFormat::Bgr24 => hwcodec::capture::CapturePixelFormat::Bgr24,
        PixelFormat::Grey => hwcodec::capture::CapturePixelFormat::Grey,
        PixelFormat::Rgb565 => hwcodec::capture::CapturePixelFormat::Unknown,
    }
}

fn map_capture_format(format: hwcodec::capture::CapturePixelFormat) -> Result<PixelFormat> {
    match format {
        hwcodec::capture::CapturePixelFormat::Mjpeg => Ok(PixelFormat::Mjpeg),
        hwcodec::capture::CapturePixelFormat::Jpeg => Ok(PixelFormat::Jpeg),
        hwcodec::capture::CapturePixelFormat::Yuyv => Ok(PixelFormat::Yuyv),
        hwcodec::capture::CapturePixelFormat::Yvyu => Ok(PixelFormat::Yvyu),
        hwcodec::capture::CapturePixelFormat::Uyvy => Ok(PixelFormat::Uyvy),
        hwcodec::capture::CapturePixelFormat::Nv12 => Ok(PixelFormat::Nv12),
        hwcodec::capture::CapturePixelFormat::Nv21 => Ok(PixelFormat::Nv21),
        hwcodec::capture::CapturePixelFormat::Nv16 => Ok(PixelFormat::Nv16),
        hwcodec::capture::CapturePixelFormat::Nv24 => Ok(PixelFormat::Nv24),
        hwcodec::capture::CapturePixelFormat::Yuv420 => Ok(PixelFormat::Yuv420),
        hwcodec::capture::CapturePixelFormat::Yvu420 => Ok(PixelFormat::Yvu420),
        hwcodec::capture::CapturePixelFormat::Rgb24 => Ok(PixelFormat::Rgb24),
        hwcodec::capture::CapturePixelFormat::Bgr24 => Ok(PixelFormat::Bgr24),
        hwcodec::capture::CapturePixelFormat::Grey => Ok(PixelFormat::Grey),
        hwcodec::capture::CapturePixelFormat::Unknown => Err(AppError::ServiceUnavailable(
            "DirectShow returned an unsupported pixel format".to_string(),
        )),
    }
}
