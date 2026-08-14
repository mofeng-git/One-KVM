//! Video device discovery, capability probing, and platform adapters.

#[cfg(unix)]
mod linux;
#[cfg(windows)]
mod windows;

#[cfg(unix)]
pub use linux::{
    enumerate_devices, find_best_device, select_recovery_device, VideoDevice, VideoDeviceInfo,
    VideoDeviceRecoveryHint,
};
#[cfg(windows)]
pub use windows::*;

use serde::{Deserialize, Serialize};

use crate::video::format::{PixelFormat, Resolution};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VideoControlMode {
    Configurable,
    SourceFollowing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VideoInputState {
    Locked,
    NoSignal,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VideoInputStatus {
    pub state: VideoInputState,
    pub format: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub fps: Option<f64>,
}

impl VideoInputStatus {
    pub fn locked(format: PixelFormat, width: u32, height: u32, fps: f64) -> Self {
        Self::locked_with_optional_fps(Some(format), width, height, Some(fps))
    }

    pub fn locked_with_optional_fps(
        format: Option<PixelFormat>,
        width: u32,
        height: u32,
        fps: Option<f64>,
    ) -> Self {
        Self {
            state: VideoInputState::Locked,
            format: format.map(|format| format.to_string()),
            width: Some(width),
            height: Some(height),
            fps,
        }
    }

    pub const fn no_signal() -> Self {
        Self {
            state: VideoInputState::NoSignal,
            format: None,
            width: None,
            height: None,
            fps: None,
        }
    }

    pub const fn unavailable() -> Self {
        Self {
            state: VideoInputState::Unavailable,
            format: None,
            width: None,
            height: None,
            fps: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ResolvedVideoInputConfig {
    pub format: PixelFormat,
    pub resolution: Resolution,
    pub fps: u32,
}

#[cfg(unix)]
pub mod bridge;
#[cfg(windows)]
#[path = "disabled_bridge.rs"]
pub mod bridge;

pub(crate) fn is_rk_hdmirx_driver(driver: &str, card: &str) -> bool {
    [driver, card].iter().any(|name| {
        name.eq_ignore_ascii_case("rk_hdmirx") || name.eq_ignore_ascii_case("snps_hdmirx")
    })
}

pub(crate) fn is_rkcif_driver(driver: &str) -> bool {
    driver.to_ascii_lowercase().starts_with("rkcif")
}

pub fn control_mode(driver: &str, card: &str) -> VideoControlMode {
    if is_rkcif_driver(driver) || is_rk_hdmirx_driver(driver, card) {
        VideoControlMode::SourceFollowing
    } else {
        VideoControlMode::Configurable
    }
}

/// Unified check for CSI/HDMI bridge devices (rk_hdmirx, rkcif, etc.)
/// that require special enumeration and format-selection logic.
pub(crate) fn is_csi_hdmi_bridge(device: &VideoDeviceInfo) -> bool {
    device.control_mode == VideoControlMode::SourceFollowing
}

pub fn resolve_video_input_config(
    device: &VideoDeviceInfo,
    requested_format: PixelFormat,
    requested_resolution: Resolution,
    requested_fps: u32,
) -> ResolvedVideoInputConfig {
    let mut resolved = ResolvedVideoInputConfig {
        format: requested_format,
        resolution: requested_resolution,
        fps: requested_fps,
    };

    if device.control_mode == VideoControlMode::SourceFollowing {
        if let VideoInputStatus {
            state: VideoInputState::Locked,
            format: Some(format),
            width: Some(width),
            height: Some(height),
            fps: Some(fps),
        } = &device.input_status
        {
            if let Ok(format) = format.parse::<PixelFormat>() {
                resolved = ResolvedVideoInputConfig {
                    format,
                    resolution: Resolution::new(*width, *height),
                    fps: fps.round().clamp(1.0, 120.0) as u32,
                };
            }
        }

        // Source-following devices do not allow One-KVM to choose the HDMI
        // resolution or frame rate, but their pixel format still has to be one
        // of the formats enumerated by the capture node.  In particular, rkcif
        // commonly exposes NV12 but One-KVM's default is MJPEG.  Passing that
        // unsupported default to S_FMT leaves the pipeline in an invalid state.
        if !device.formats.is_empty()
            && !device
                .formats
                .iter()
                .any(|format| format.format == resolved.format)
        {
            resolved.format = device.formats[0].format;
        }
    }

    resolved
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    use super::linux::FormatInfo;

    #[cfg(unix)]
    fn device(control_mode: VideoControlMode, input_status: VideoInputStatus) -> VideoDeviceInfo {
        VideoDeviceInfo {
            path: "/dev/video0".into(),
            name: "test".into(),
            driver: "test".into(),
            bus_info: "test".into(),
            card: "test".into(),
            formats: Vec::new(),
            capabilities: Default::default(),
            is_capture_card: true,
            priority: 0,
            has_signal: input_status.state == VideoInputState::Locked,
            control_mode,
            input_status,
            subdev_path: None,
            bridge_kind: None,
        }
    }

    #[cfg(unix)]
    fn format(format: PixelFormat) -> FormatInfo {
        FormatInfo {
            format,
            resolutions: Vec::new(),
            description: format.to_string(),
        }
    }

    #[test]
    fn recognizes_vendor_and_upstream_native_hdmirx_names() {
        assert!(is_rk_hdmirx_driver("rk_hdmirx", "rk_hdmirx"));
        assert!(is_rk_hdmirx_driver("snps_hdmirx", "Synopsys HDMI RX"));
        assert!(is_rk_hdmirx_driver("other", "SNPS_HDMIRX"));
        assert!(!is_rk_hdmirx_driver("rkcif", "stream_cif_mipi_id0"));
    }

    #[test]
    fn classifies_source_following_drivers_in_one_place() {
        assert_eq!(
            control_mode("rkcif", "stream_cif_mipi_id0"),
            VideoControlMode::SourceFollowing
        );
        assert_eq!(
            control_mode("rkcif-mipi", "capture"),
            VideoControlMode::SourceFollowing
        );
        assert_eq!(
            control_mode("rk_hdmirx", "capture"),
            VideoControlMode::SourceFollowing
        );
        assert_eq!(
            control_mode("uvcvideo", "USB Capture"),
            VideoControlMode::Configurable
        );
    }

    #[cfg(unix)]
    #[test]
    fn source_following_uses_locked_hardware_mode_and_exact_fps_rounding() {
        let device = device(
            VideoControlMode::SourceFollowing,
            VideoInputStatus::locked(PixelFormat::Nv12, 1920, 1080, 59.94),
        );
        let resolved = resolve_video_input_config(
            &device,
            PixelFormat::Mjpeg,
            Resolution::new(3840, 2160),
            15,
        );
        assert_eq!(resolved.format, PixelFormat::Nv12);
        assert_eq!(resolved.resolution, Resolution::new(1920, 1080));
        assert_eq!(resolved.fps, 60);
    }

    #[cfg(unix)]
    #[test]
    fn no_signal_keeps_fallback_and_configurable_keeps_request() {
        for (mode, status) in [
            (
                VideoControlMode::SourceFollowing,
                VideoInputStatus::no_signal(),
            ),
            (
                VideoControlMode::Configurable,
                VideoInputStatus::unavailable(),
            ),
        ] {
            let resolved = resolve_video_input_config(
                &device(mode, status),
                PixelFormat::Yuyv,
                Resolution::new(1280, 720),
                30,
            );
            assert_eq!(resolved.format, PixelFormat::Yuyv);
            assert_eq!(resolved.resolution, Resolution::new(1280, 720));
            assert_eq!(resolved.fps, 30);
        }
    }

    #[cfg(unix)]
    #[test]
    fn source_following_replaces_unenumerated_default_format_without_signal() {
        let mut device = device(
            VideoControlMode::SourceFollowing,
            VideoInputStatus::no_signal(),
        );
        device.formats = vec![format(PixelFormat::Nv12), format(PixelFormat::Yuyv)];

        let resolved = resolve_video_input_config(
            &device,
            PixelFormat::Mjpeg,
            Resolution::new(1920, 1080),
            30,
        );

        assert_eq!(resolved.format, PixelFormat::Nv12);
        assert_eq!(resolved.resolution, Resolution::new(1920, 1080));
        assert_eq!(resolved.fps, 30);
    }

    #[cfg(unix)]
    #[test]
    fn source_following_replaces_stale_active_format_but_keeps_input_mode() {
        let mut device = device(
            VideoControlMode::SourceFollowing,
            VideoInputStatus::locked(PixelFormat::Mjpeg, 1280, 720, 59.94),
        );
        device.formats = vec![format(PixelFormat::Nv12), format(PixelFormat::Yuyv)];

        let resolved = resolve_video_input_config(
            &device,
            PixelFormat::Mjpeg,
            Resolution::new(1920, 1080),
            30,
        );

        assert_eq!(resolved.format, PixelFormat::Nv12);
        assert_eq!(resolved.resolution, Resolution::new(1280, 720));
        assert_eq!(resolved.fps, 60);
    }

    #[test]
    fn no_signal_and_unavailable_never_expose_stale_mode_fields() {
        for status in [
            VideoInputStatus::no_signal(),
            VideoInputStatus::unavailable(),
        ] {
            assert!(status.format.is_none());
            assert!(status.width.is_none());
            assert!(status.height.is_none());
            assert!(status.fps.is_none());
        }
    }
}

#[cfg(unix)]
pub(crate) use linux::parse_bridge_kind;
