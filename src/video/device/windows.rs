use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::error::{AppError, Result};
use crate::video::format::{PixelFormat, Resolution};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoDeviceInfo {
    pub path: PathBuf,
    pub name: String,
    pub driver: String,
    pub bus_info: String,
    pub card: String,
    pub formats: Vec<FormatInfo>,
    pub capabilities: DeviceCapabilities,
    pub is_capture_card: bool,
    pub priority: u32,
    pub has_signal: bool,
    pub subdev_path: Option<PathBuf>,
    pub bridge_kind: Option<String>,
}

#[derive(Debug, Clone)]
pub struct VideoDeviceRecoveryHint {
    pub path: PathBuf,
    pub name: String,
    pub driver: String,
    pub bus_info: String,
    pub card: String,
    pub is_capture_card: bool,
}

impl From<&VideoDeviceInfo> for VideoDeviceRecoveryHint {
    fn from(device: &VideoDeviceInfo) -> Self {
        Self {
            path: device.path.clone(),
            name: device.name.clone(),
            driver: device.driver.clone(),
            bus_info: device.bus_info.clone(),
            card: device.card.clone(),
            is_capture_card: device.is_capture_card,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormatInfo {
    pub format: PixelFormat,
    pub resolutions: Vec<ResolutionInfo>,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolutionInfo {
    pub width: u32,
    pub height: u32,
    pub fps: Vec<f64>,
}

impl ResolutionInfo {
    pub fn new(width: u32, height: u32, fps: Vec<f64>) -> Self {
        Self { width, height, fps }
    }

    pub fn resolution(&self) -> Resolution {
        Resolution::new(self.width, self.height)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DeviceCapabilities {
    pub video_capture: bool,
    pub video_capture_mplane: bool,
    pub video_output: bool,
    pub streaming: bool,
    pub read_write: bool,
}

pub struct VideoDevice {
    pub path: PathBuf,
}

pub(crate) const DIRECTSHOW_DEVICE_PREFIX: &str = "dshow:";

impl VideoDevice {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = normalize_windows_device_path(path.as_ref());
        if enumerate_devices()?
            .iter()
            .any(|device| device.path == path)
        {
            Ok(Self { path })
        } else {
            Err(AppError::VideoError(format!(
                "Windows video device not found: {}",
                path.display()
            )))
        }
    }

    pub fn open_readonly(path: impl AsRef<Path>) -> Result<Self> {
        Self::open(path)
    }

    pub fn info(&self) -> Result<VideoDeviceInfo> {
        enumerate_devices()?
            .into_iter()
            .find(|device| device.path == self.path)
            .ok_or_else(|| {
                AppError::VideoError(format!(
                    "Windows video device not found: {}",
                    self.path.display()
                ))
            })
    }
}

pub(crate) fn normalize_windows_device_path(path: impl AsRef<Path>) -> PathBuf {
    let raw = path.as_ref().to_string_lossy();
    if raw.eq_ignore_ascii_case("auto") {
        return find_best_device()
            .map(|device| device.path)
            .unwrap_or_else(|_| PathBuf::from(raw.as_ref()));
    }
    PathBuf::from(raw.as_ref())
}

pub(crate) fn directshow_display_name_from_path(path: impl AsRef<Path>) -> Option<String> {
    path.as_ref()
        .to_string_lossy()
        .strip_prefix(DIRECTSHOW_DEVICE_PREFIX)
        .map(str::to_string)
}

pub fn enumerate_devices() -> Result<Vec<VideoDeviceInfo>> {
    let names = hwcodec::capture::list_dshow_video_devices().map_err(|e| {
        AppError::VideoError(format!("Failed to enumerate DirectShow devices: {}", e))
    })?;

    let mut devices = names
        .into_iter()
        .enumerate()
        .map(|(index, name)| directshow_device_from_name(index, name))
        .collect::<Vec<_>>();

    devices.sort_by(|a, b| {
        b.priority
            .cmp(&a.priority)
            .then_with(|| a.name.cmp(&b.name))
    });
    Ok(devices)
}

pub fn find_best_device() -> Result<VideoDeviceInfo> {
    enumerate_devices()?.into_iter().next().ok_or_else(|| {
        AppError::VideoError("No DirectShow video capture devices found".to_string())
    })
}

pub fn parse_bridge_kind(value: Option<&str>) -> Option<super::bridge::CsiBridgeKind> {
    value.and_then(|_| None)
}

pub fn select_recovery_device(
    devices: &[VideoDeviceInfo],
    hint: &VideoDeviceRecoveryHint,
) -> Option<VideoDeviceInfo> {
    devices
        .iter()
        .find(|device| device.path == hint.path || device.bus_info == hint.bus_info)
        .cloned()
}

fn directshow_device_from_name(index: usize, name: String) -> VideoDeviceInfo {
    let name = if name.trim().is_empty() {
        format!("Windows Capture Device {}", index + 1)
    } else {
        name
    };
    let path = PathBuf::from(format!("{}{}", DIRECTSHOW_DEVICE_PREFIX, name));
    let formats = enumerate_directshow_formats(&name);
    let priority = score_capture_device(&name, &path.to_string_lossy(), &formats);

    VideoDeviceInfo {
        path,
        name: name.clone(),
        driver: "directshow".to_string(),
        bus_info: name.clone(),
        card: name,
        formats,
        capabilities: DeviceCapabilities {
            video_capture: true,
            video_capture_mplane: false,
            video_output: false,
            streaming: true,
            read_write: false,
        },
        is_capture_card: true,
        priority,
        has_signal: true,
        subdev_path: None,
        bridge_kind: None,
    }
}

fn enumerate_directshow_formats(name: &str) -> Vec<FormatInfo> {
    let Ok(capabilities) = hwcodec::capture::list_dshow_device_capabilities(name) else {
        return fallback_windows_formats();
    };

    let mut formats: Vec<FormatInfo> = Vec::new();
    for capability in capabilities {
        let Some(format) = map_capture_format(capability.format) else {
            continue;
        };
        if capability.width == 0 || capability.height == 0 {
            continue;
        }

        if let Some(existing) = formats.iter_mut().find(|info| info.format == format) {
            merge_resolution(
                &mut existing.resolutions,
                capability.width,
                capability.height,
                &capability.fps,
            );
            continue;
        }

        let mut resolutions = Vec::new();
        merge_resolution(
            &mut resolutions,
            capability.width,
            capability.height,
            &capability.fps,
        );
        formats.push(FormatInfo {
            format,
            resolutions,
            description: format_description(format).to_string(),
        });
    }

    for info in &mut formats {
        info.resolutions.sort_by(|left, right| {
            b_pixels(right)
                .cmp(&b_pixels(left))
                .then_with(|| right.width.cmp(&left.width))
                .then_with(|| right.height.cmp(&left.height))
        });
    }
    formats.sort_by(|a, b| {
        b.format
            .priority()
            .cmp(&a.format.priority())
            .then_with(|| a.description.cmp(&b.description))
    });

    if formats.is_empty() {
        fallback_windows_formats()
    } else {
        formats
    }
}

fn merge_resolution(resolutions: &mut Vec<ResolutionInfo>, width: u32, height: u32, fps: &[u32]) {
    if let Some(existing) = resolutions
        .iter_mut()
        .find(|resolution| resolution.width == width && resolution.height == height)
    {
        existing.fps.extend(fps.iter().map(|value| *value as f64));
        normalize_fps_list(&mut existing.fps);
        return;
    }

    let mut fps_values = fps.iter().map(|value| *value as f64).collect::<Vec<_>>();
    normalize_fps_list(&mut fps_values);
    resolutions.push(ResolutionInfo::new(width, height, fps_values));
}

fn b_pixels(resolution: &ResolutionInfo) -> u32 {
    resolution.width.saturating_mul(resolution.height)
}

fn map_capture_format(format: hwcodec::capture::CapturePixelFormat) -> Option<PixelFormat> {
    match format {
        hwcodec::capture::CapturePixelFormat::Mjpeg => Some(PixelFormat::Mjpeg),
        hwcodec::capture::CapturePixelFormat::Jpeg => Some(PixelFormat::Jpeg),
        hwcodec::capture::CapturePixelFormat::Yuyv => Some(PixelFormat::Yuyv),
        hwcodec::capture::CapturePixelFormat::Yvyu => Some(PixelFormat::Yvyu),
        hwcodec::capture::CapturePixelFormat::Uyvy => Some(PixelFormat::Uyvy),
        hwcodec::capture::CapturePixelFormat::Nv12 => Some(PixelFormat::Nv12),
        hwcodec::capture::CapturePixelFormat::Nv21 => Some(PixelFormat::Nv21),
        hwcodec::capture::CapturePixelFormat::Nv16 => Some(PixelFormat::Nv16),
        hwcodec::capture::CapturePixelFormat::Nv24 => Some(PixelFormat::Nv24),
        hwcodec::capture::CapturePixelFormat::Yuv420 => Some(PixelFormat::Yuv420),
        hwcodec::capture::CapturePixelFormat::Yvu420 => Some(PixelFormat::Yvu420),
        hwcodec::capture::CapturePixelFormat::Rgb24 => Some(PixelFormat::Rgb24),
        hwcodec::capture::CapturePixelFormat::Bgr24 => Some(PixelFormat::Bgr24),
        hwcodec::capture::CapturePixelFormat::Grey => Some(PixelFormat::Grey),
        hwcodec::capture::CapturePixelFormat::Unknown => None,
    }
}

fn normalize_fps_list(fps_list: &mut Vec<f64>) {
    fps_list.retain(|fps| fps.is_finite() && *fps > 0.0);
    for fps in fps_list.iter_mut() {
        *fps = (*fps * 100.0).round() / 100.0;
    }
    fps_list.sort_by(|a, b| b.total_cmp(a));
    fps_list.dedup_by(|a, b| (*a - *b).abs() < 0.01);
}

fn format_description(format: PixelFormat) -> &'static str {
    match format {
        PixelFormat::Mjpeg => "MJPEG",
        PixelFormat::Jpeg => "JPEG",
        PixelFormat::Yuyv => "YUYV 4:2:2",
        PixelFormat::Yvyu => "YVYU 4:2:2",
        PixelFormat::Uyvy => "UYVY 4:2:2",
        PixelFormat::Nv12 => "NV12",
        PixelFormat::Nv21 => "NV21",
        PixelFormat::Nv16 => "NV16",
        PixelFormat::Nv24 => "NV24",
        PixelFormat::Yuv420 => "YUV420",
        PixelFormat::Yvu420 => "YVU420",
        PixelFormat::Rgb565 => "RGB565",
        PixelFormat::Rgb24 => "RGB24",
        PixelFormat::Bgr24 => "BGR24",
        PixelFormat::Grey => "GREY",
    }
}

fn score_capture_device(name: &str, device_id: &str, formats: &[FormatInfo]) -> u32 {
    let haystack = format!("{} {}", name, device_id).to_ascii_lowercase();
    let mut score = 50;

    if formats
        .iter()
        .any(|format| format.format == PixelFormat::Mjpeg)
    {
        score += 25;
    }
    for keyword in ["capture", "hdmi", "uvc", "video", "usb"] {
        if haystack.contains(keyword) {
            score += 10;
        }
    }

    score
}

fn fallback_windows_formats() -> Vec<FormatInfo> {
    vec![FormatInfo {
        format: PixelFormat::Mjpeg,
        resolutions: Vec::new(),
        description: "DirectShow auto-detected stream format".to_string(),
    }]
}
