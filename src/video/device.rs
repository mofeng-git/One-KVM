//! V4L2 device enumeration and capability query

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};
use v4l::capability::Flags;
use v4l::prelude::*;
use v4l::video::Capture;
use v4l::Format;
use v4l::FourCC;

use super::format::{PixelFormat, Resolution};
use crate::error::{AppError, Result};

/// Information about a video device
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoDeviceInfo {
    /// Device path (e.g., /dev/video0)
    pub path: PathBuf,
    /// Device name from driver
    pub name: String,
    /// Driver name
    pub driver: String,
    /// Bus info
    pub bus_info: String,
    /// Card name
    pub card: String,
    /// Supported pixel formats
    pub formats: Vec<FormatInfo>,
    /// Device capabilities
    pub capabilities: DeviceCapabilities,
    /// Whether this is likely an HDMI capture card
    pub is_capture_card: bool,
    /// Priority score for device selection (higher is better)
    pub priority: u32,
}

/// Information about a supported format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormatInfo {
    /// Pixel format
    pub format: PixelFormat,
    /// Supported resolutions
    pub resolutions: Vec<ResolutionInfo>,
    /// Description from driver
    pub description: String,
}

/// Information about a supported resolution and frame rates
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolutionInfo {
    pub width: u32,
    pub height: u32,
    pub fps: Vec<u32>,
}

impl ResolutionInfo {
    pub fn new(width: u32, height: u32, fps: Vec<u32>) -> Self {
        Self { width, height, fps }
    }

    pub fn resolution(&self) -> Resolution {
        Resolution::new(self.width, self.height)
    }
}

/// Device capabilities
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DeviceCapabilities {
    pub video_capture: bool,
    pub video_capture_mplane: bool,
    pub video_output: bool,
    pub streaming: bool,
    pub read_write: bool,
}

/// Wrapper around a V4L2 video device
pub struct VideoDevice {
    pub path: PathBuf,
    device: Device,
}

impl VideoDevice {
    /// Open a video device by path
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        debug!("Opening video device: {:?}", path);

        let device = Device::with_path(&path).map_err(|e| {
            AppError::VideoError(format!("Failed to open device {:?}: {}", path, e))
        })?;

        Ok(Self { path, device })
    }

    /// Get device capabilities
    pub fn capabilities(&self) -> Result<DeviceCapabilities> {
        let caps = self.device.query_caps().map_err(|e| {
            AppError::VideoError(format!("Failed to query capabilities: {}", e))
        })?;

        Ok(DeviceCapabilities {
            video_capture: caps.capabilities.contains(Flags::VIDEO_CAPTURE),
            video_capture_mplane: caps.capabilities.contains(Flags::VIDEO_CAPTURE_MPLANE),
            video_output: caps.capabilities.contains(Flags::VIDEO_OUTPUT),
            streaming: caps.capabilities.contains(Flags::STREAMING),
            read_write: caps.capabilities.contains(Flags::READ_WRITE),
        })
    }

    /// Get detailed device information
    pub fn info(&self) -> Result<VideoDeviceInfo> {
        let caps = self.device.query_caps().map_err(|e| {
            AppError::VideoError(format!("Failed to query capabilities: {}", e))
        })?;

        let capabilities = DeviceCapabilities {
            video_capture: caps.capabilities.contains(Flags::VIDEO_CAPTURE),
            video_capture_mplane: caps.capabilities.contains(Flags::VIDEO_CAPTURE_MPLANE),
            video_output: caps.capabilities.contains(Flags::VIDEO_OUTPUT),
            streaming: caps.capabilities.contains(Flags::STREAMING),
            read_write: caps.capabilities.contains(Flags::READ_WRITE),
        };

        let formats = self.enumerate_formats()?;

        // Determine if this is likely an HDMI capture card
        let is_capture_card = Self::detect_capture_card(&caps.card, &caps.driver, &formats);

        // Calculate priority score
        let priority = Self::calculate_priority(&caps.card, &caps.driver, &formats, is_capture_card);

        Ok(VideoDeviceInfo {
            path: self.path.clone(),
            name: caps.card.clone(),
            driver: caps.driver.clone(),
            bus_info: caps.bus.clone(),
            card: caps.card,
            formats,
            capabilities,
            is_capture_card,
            priority,
        })
    }

    /// Enumerate supported formats
    pub fn enumerate_formats(&self) -> Result<Vec<FormatInfo>> {
        let mut formats = Vec::new();

        // Get supported formats
        let format_descs = self.device.enum_formats().map_err(|e| {
            AppError::VideoError(format!("Failed to enumerate formats: {}", e))
        })?;

        for desc in format_descs {
            // Try to convert FourCC to our PixelFormat
            if let Some(format) = PixelFormat::from_fourcc(desc.fourcc) {
                let resolutions = self.enumerate_resolutions(desc.fourcc)?;

                formats.push(FormatInfo {
                    format,
                    resolutions,
                    description: desc.description.clone(),
                });
            } else {
                debug!(
                    "Skipping unsupported format: {:?} ({})",
                    desc.fourcc, desc.description
                );
            }
        }

        // Sort by format priority (MJPEG first)
        formats.sort_by(|a, b| b.format.priority().cmp(&a.format.priority()));

        Ok(formats)
    }

    /// Enumerate resolutions for a specific format
    fn enumerate_resolutions(&self, fourcc: FourCC) -> Result<Vec<ResolutionInfo>> {
        let mut resolutions = Vec::new();

        // Try to enumerate frame sizes
        match self.device.enum_framesizes(fourcc) {
            Ok(sizes) => {
                for size in sizes {
                    match size.size {
                        v4l::framesize::FrameSizeEnum::Discrete(d) => {
                            let fps = self.enumerate_fps(fourcc, d.width, d.height).unwrap_or_default();
                            resolutions.push(ResolutionInfo::new(d.width, d.height, fps));
                        }
                        v4l::framesize::FrameSizeEnum::Stepwise(s) => {
                            // For stepwise, add some common resolutions
                            for res in [
                                Resolution::VGA,
                                Resolution::HD720,
                                Resolution::HD1080,
                                Resolution::UHD4K,
                            ] {
                                if res.width >= s.min_width
                                    && res.width <= s.max_width
                                    && res.height >= s.min_height
                                    && res.height <= s.max_height
                                {
                                    let fps = self.enumerate_fps(fourcc, res.width, res.height).unwrap_or_default();
                                    resolutions.push(ResolutionInfo::new(res.width, res.height, fps));
                                }
                            }
                        }
                    }
                }
            }
            Err(e) => {
                debug!("Failed to enumerate frame sizes for {:?}: {}", fourcc, e);
            }
        }

        // Sort by resolution (largest first)
        resolutions.sort_by(|a, b| (b.width * b.height).cmp(&(a.width * a.height)));
        resolutions.dedup_by(|a, b| a.width == b.width && a.height == b.height);

        Ok(resolutions)
    }

    /// Enumerate FPS for a specific resolution
    fn enumerate_fps(&self, fourcc: FourCC, width: u32, height: u32) -> Result<Vec<u32>> {
        let mut fps_list = Vec::new();

        match self.device.enum_frameintervals(fourcc, width, height) {
            Ok(intervals) => {
                for interval in intervals {
                    match interval.interval {
                        v4l::frameinterval::FrameIntervalEnum::Discrete(fraction) => {
                            if fraction.numerator > 0 {
                                let fps = fraction.denominator / fraction.numerator;
                                fps_list.push(fps);
                            }
                        }
                        v4l::frameinterval::FrameIntervalEnum::Stepwise(step) => {
                            // Just pick max/min/step
                            if step.max.numerator > 0 {
                                let min_fps = step.max.denominator / step.max.numerator;
                                let max_fps = step.min.denominator / step.min.numerator;
                                fps_list.push(min_fps);
                                if max_fps != min_fps {
                                    fps_list.push(max_fps);
                                }
                            }
                        }
                    }
                }
            }
            Err(_) => {
                // If enumeration fails, assume 30fps
                fps_list.push(30);
            }
        }
        
        fps_list.sort_by(|a, b| b.cmp(a));
        fps_list.dedup();
        Ok(fps_list)
    }

    /// Get current format
    pub fn get_format(&self) -> Result<Format> {
        self.device.format().map_err(|e| {
            AppError::VideoError(format!("Failed to get format: {}", e))
        })
    }

    /// Set capture format
    pub fn set_format(&self, width: u32, height: u32, format: PixelFormat) -> Result<Format> {
        let fmt = Format::new(width, height, format.to_fourcc());

        // Request the format
        let actual = self.device.set_format(&fmt).map_err(|e| {
            AppError::VideoError(format!("Failed to set format: {}", e))
        })?;

        if actual.width != width || actual.height != height {
            warn!(
                "Requested {}x{}, got {}x{}",
                width, height, actual.width, actual.height
            );
        }

        Ok(actual)
    }

    /// Detect if device is likely an HDMI capture card
    fn detect_capture_card(card: &str, driver: &str, formats: &[FormatInfo]) -> bool {
        let card_lower = card.to_lowercase();
        let driver_lower = driver.to_lowercase();

        // Known capture card patterns
        let capture_patterns = [
            "hdmi",
            "capture",
            "grabber",
            "usb3",
            "ms2109",
            "ms2130",
            "macrosilicon",
            "tc358743",
            "uvc",
        ];

        // Check card/driver names
        for pattern in capture_patterns {
            if card_lower.contains(pattern) || driver_lower.contains(pattern) {
                return true;
            }
        }

        // Capture cards usually support MJPEG and high resolutions
        let has_mjpeg = formats.iter().any(|f| f.format == PixelFormat::Mjpeg);
        let has_1080p = formats.iter().any(|f| {
            f.resolutions
                .iter()
                .any(|r| r.width >= 1920 && r.height >= 1080)
        });

        has_mjpeg && has_1080p
    }

    /// Calculate device priority for selection
    fn calculate_priority(
        _card: &str,
        driver: &str,
        formats: &[FormatInfo],
        is_capture_card: bool,
    ) -> u32 {
        let mut priority = 0u32;

        // Capture cards get highest priority
        if is_capture_card {
            priority += 1000;
        }

        // MJPEG support is valuable
        if formats.iter().any(|f| f.format == PixelFormat::Mjpeg) {
            priority += 100;
        }

        // High resolution support
        let max_resolution = formats
            .iter()
            .flat_map(|f| &f.resolutions)
            .map(|r| r.width * r.height)
            .max()
            .unwrap_or(0);

        priority += (max_resolution / 100000) as u32;

        // Known good drivers get bonus
        let good_drivers = ["uvcvideo", "tc358743"];
        if good_drivers.iter().any(|d| driver.contains(d)) {
            priority += 50;
        }

        priority
    }

    /// Get the inner device reference (for advanced usage)
    pub fn inner(&self) -> &Device {
        &self.device
    }
}

/// Enumerate all video capture devices
pub fn enumerate_devices() -> Result<Vec<VideoDeviceInfo>> {
    info!("Enumerating video devices...");

    let mut devices = Vec::new();

    // Scan /dev/video* devices
    for entry in std::fs::read_dir("/dev").map_err(|e| {
        AppError::VideoError(format!("Failed to read /dev: {}", e))
    })? {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };

        let path = entry.path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

        if !name.starts_with("video") {
            continue;
        }

        debug!("Found video device: {:?}", path);

        // Try to open and query the device
        match VideoDevice::open(&path) {
            Ok(device) => {
                match device.info() {
                    Ok(info) => {
                        // Only include devices with video capture capability
                        if info.capabilities.video_capture || info.capabilities.video_capture_mplane
                        {
                            info!(
                                "Found capture device: {} ({}) - {} formats",
                                info.name,
                                info.driver,
                                info.formats.len()
                            );
                            devices.push(info);
                        } else {
                            debug!("Skipping non-capture device: {:?}", path);
                        }
                    }
                    Err(e) => {
                        debug!("Failed to get info for {:?}: {}", path, e);
                    }
                }
            }
            Err(e) => {
                debug!("Failed to open {:?}: {}", path, e);
            }
        }
    }

    // Sort by priority (highest first)
    devices.sort_by(|a, b| b.priority.cmp(&a.priority));

    info!("Found {} video capture devices", devices.len());
    Ok(devices)
}

/// Find the best video device for KVM use
pub fn find_best_device() -> Result<VideoDeviceInfo> {
    let devices = enumerate_devices()?;

    devices.into_iter().next().ok_or_else(|| {
        AppError::VideoError("No video capture devices found".to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pixel_format_conversion() {
        let format = PixelFormat::Mjpeg;
        let fourcc = format.to_fourcc();
        let back = PixelFormat::from_fourcc(fourcc);
        assert_eq!(back, Some(format));
    }

    #[test]
    fn test_resolution() {
        let res = Resolution::HD1080;
        assert_eq!(res.width, 1920);
        assert_eq!(res.height, 1080);
        assert!(res.is_valid());
    }
}
