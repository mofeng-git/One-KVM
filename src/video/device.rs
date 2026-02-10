//! V4L2 device enumeration and capability query

use serde::{Deserialize, Serialize};
use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;
use tracing::{debug, info, warn};
use v4l2r::nix::errno::Errno;
use v4l2r::bindings::{v4l2_frmivalenum, v4l2_frmsizeenum};
use v4l2r::ioctl::{
    self, Capabilities, Capability as V4l2rCapability, FormatIterator, FrmIvalTypes, FrmSizeTypes,
};
use v4l2r::{Format as V4l2rFormat, QueueType};

use super::format::{PixelFormat, Resolution};
use crate::error::{AppError, Result};

const DEVICE_PROBE_TIMEOUT_MS: u64 = 400;

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
    fd: File,
}

impl VideoDevice {
    /// Open a video device by path
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        debug!("Opening video device: {:?}", path);

        let fd = File::options()
            .read(true)
            .write(true)
            .open(&path)
            .map_err(|e| AppError::VideoError(format!("Failed to open device {:?}: {}", path, e)))?;

        Ok(Self { path, fd })
    }

    /// Open a video device read-only (for probing/enumeration)
    pub fn open_readonly(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        debug!("Opening video device (read-only): {:?}", path);

        let fd = File::options()
            .read(true)
            .open(&path)
            .map_err(|e| AppError::VideoError(format!("Failed to open device {:?}: {}", path, e)))?;

        Ok(Self { path, fd })
    }

    /// Get device capabilities
    pub fn capabilities(&self) -> Result<DeviceCapabilities> {
        let caps: V4l2rCapability = ioctl::querycap(&self.fd)
            .map_err(|e| AppError::VideoError(format!("Failed to query capabilities: {}", e)))?;
        let flags = caps.device_caps();

        Ok(DeviceCapabilities {
            video_capture: flags.contains(Capabilities::VIDEO_CAPTURE),
            video_capture_mplane: flags.contains(Capabilities::VIDEO_CAPTURE_MPLANE),
            video_output: flags.contains(Capabilities::VIDEO_OUTPUT),
            streaming: flags.contains(Capabilities::STREAMING),
            read_write: flags.contains(Capabilities::READWRITE),
        })
    }

    /// Get detailed device information
    pub fn info(&self) -> Result<VideoDeviceInfo> {
        let caps: V4l2rCapability = ioctl::querycap(&self.fd)
            .map_err(|e| AppError::VideoError(format!("Failed to query capabilities: {}", e)))?;
        let flags = caps.device_caps();
        let capabilities = DeviceCapabilities {
            video_capture: flags.contains(Capabilities::VIDEO_CAPTURE),
            video_capture_mplane: flags.contains(Capabilities::VIDEO_CAPTURE_MPLANE),
            video_output: flags.contains(Capabilities::VIDEO_OUTPUT),
            streaming: flags.contains(Capabilities::STREAMING),
            read_write: flags.contains(Capabilities::READWRITE),
        };

        let formats = self.enumerate_formats()?;

        // Determine if this is likely an HDMI capture card
        let is_capture_card = Self::detect_capture_card(&caps.card, &caps.driver, &formats);

        // Calculate priority score
        let priority =
            Self::calculate_priority(&caps.card, &caps.driver, &formats, is_capture_card);

        Ok(VideoDeviceInfo {
            path: self.path.clone(),
            name: caps.card.clone(),
            driver: caps.driver.clone(),
            bus_info: caps.bus_info.clone(),
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

        let queue = self.capture_queue_type()?;
        let format_descs = FormatIterator::new(&self.fd, queue);

        for desc in format_descs {
            // Try to convert FourCC to our PixelFormat
            if let Some(format) = PixelFormat::from_v4l2r(desc.pixelformat) {
                let resolutions = self.enumerate_resolutions(desc.pixelformat)?;

                formats.push(FormatInfo {
                    format,
                    resolutions,
                    description: desc.description.clone(),
                });
            } else {
                debug!(
                    "Skipping unsupported format: {:?} ({})",
                    desc.pixelformat, desc.description
                );
            }
        }

        // Sort by format priority (MJPEG first)
        formats.sort_by(|a, b| b.format.priority().cmp(&a.format.priority()));

        Ok(formats)
    }

    /// Enumerate resolutions for a specific format
    fn enumerate_resolutions(&self, fourcc: v4l2r::PixelFormat) -> Result<Vec<ResolutionInfo>> {
        let mut resolutions = Vec::new();

        let mut index = 0u32;
        loop {
            match ioctl::enum_frame_sizes::<v4l2_frmsizeenum>(&self.fd, index, fourcc) {
                Ok(size) => {
                    if let Some(size) = size.size() {
                        match size {
                            FrmSizeTypes::Discrete(d) => {
                                let fps =
                                    self.enumerate_fps(fourcc, d.width, d.height).unwrap_or_default();
                                resolutions.push(ResolutionInfo::new(d.width, d.height, fps));
                            }
                            FrmSizeTypes::StepWise(s) => {
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
                                        let fps = self
                                            .enumerate_fps(fourcc, res.width, res.height)
                                            .unwrap_or_default();
                                        resolutions.push(ResolutionInfo::new(res.width, res.height, fps));
                                    }
                                }
                            }
                        }
                    }
                    index += 1;
                }
                Err(e) => {
                    let is_einval = matches!(
                        e,
                        v4l2r::ioctl::FrameSizeError::IoctlError(err) if err == Errno::EINVAL
                    );
                    if !is_einval {
                        debug!("Failed to enumerate frame sizes for {:?}: {}", fourcc, e);
                    }
                    break;
                }
            }
        }

        // Sort by resolution (largest first)
        resolutions.sort_by(|a, b| (b.width * b.height).cmp(&(a.width * a.height)));
        resolutions.dedup_by(|a, b| a.width == b.width && a.height == b.height);

        Ok(resolutions)
    }

    /// Enumerate FPS for a specific resolution
    fn enumerate_fps(
        &self,
        fourcc: v4l2r::PixelFormat,
        width: u32,
        height: u32,
    ) -> Result<Vec<u32>> {
        let mut fps_list = Vec::new();

        let mut index = 0u32;
        loop {
            match ioctl::enum_frame_intervals::<v4l2_frmivalenum>(
                &self.fd,
                index,
                fourcc,
                width,
                height,
            ) {
                Ok(interval) => {
                    if let Some(interval) = interval.intervals() {
                        match interval {
                            FrmIvalTypes::Discrete(fraction) => {
                                if fraction.numerator > 0 {
                                    let fps = fraction.denominator / fraction.numerator;
                                    fps_list.push(fps);
                                }
                            }
                            FrmIvalTypes::StepWise(step) => {
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
                    index += 1;
                }
                Err(e) => {
                    let is_einval = matches!(
                        e,
                        v4l2r::ioctl::FrameIntervalsError::IoctlError(err) if err == Errno::EINVAL
                    );
                    if !is_einval {
                        debug!(
                            "Failed to enumerate frame intervals for {:?} {}x{}: {}",
                            fourcc, width, height, e
                        );
                    }
                    break;
                }
            }
        }

        fps_list.sort_by(|a, b| b.cmp(a));
        fps_list.dedup();
        Ok(fps_list)
    }

    /// Get current format
    pub fn get_format(&self) -> Result<V4l2rFormat> {
        let queue = self.capture_queue_type()?;
        ioctl::g_fmt(&self.fd, queue)
            .map_err(|e| AppError::VideoError(format!("Failed to get format: {}", e)))
    }

    /// Set capture format
    pub fn set_format(&self, width: u32, height: u32, format: PixelFormat) -> Result<V4l2rFormat> {
        let queue = self.capture_queue_type()?;
        let mut fmt: V4l2rFormat = ioctl::g_fmt(&self.fd, queue)
            .map_err(|e| AppError::VideoError(format!("Failed to get format: {}", e)))?;
        fmt.width = width;
        fmt.height = height;
        fmt.pixelformat = format.to_v4l2r();

        let mut fd = self
            .fd
            .try_clone()
            .map_err(|e| AppError::VideoError(format!("Failed to clone device fd: {}", e)))?;
        let actual: V4l2rFormat = ioctl::s_fmt(&mut fd, (queue, &fmt))
            .map_err(|e| AppError::VideoError(format!("Failed to set format: {}", e)))?;

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
    pub fn inner(&self) -> &File {
        &self.fd
    }

    fn capture_queue_type(&self) -> Result<QueueType> {
        let caps = self.capabilities()?;
        if caps.video_capture {
            Ok(QueueType::VideoCapture)
        } else if caps.video_capture_mplane {
            Ok(QueueType::VideoCaptureMplane)
        } else {
            Err(AppError::VideoError(
                "Device does not expose a capture queue".to_string(),
            ))
        }
    }
}

/// Enumerate all video capture devices
pub fn enumerate_devices() -> Result<Vec<VideoDeviceInfo>> {
    info!("Enumerating video devices...");

    let mut devices = Vec::new();

    // Scan /dev/video* devices
    for entry in std::fs::read_dir("/dev")
        .map_err(|e| AppError::VideoError(format!("Failed to read /dev: {}", e)))?
    {
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

        if !sysfs_maybe_capture(&path) {
            debug!("Skipping non-capture candidate (sysfs): {:?}", path);
            continue;
        }

        // Try to open and query the device (with timeout)
        match probe_device_with_timeout(&path, Duration::from_millis(DEVICE_PROBE_TIMEOUT_MS)) {
            Some(info) => {
                // Only include devices with video capture capability
                if info.capabilities.video_capture || info.capabilities.video_capture_mplane {
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
            None => {
                debug!("Failed to probe {:?}", path);
            }
        }
    }

    // Sort by priority (highest first)
    devices.sort_by(|a, b| b.priority.cmp(&a.priority));

    info!("Found {} video capture devices", devices.len());
    Ok(devices)
}

fn probe_device_with_timeout(path: &Path, timeout: Duration) -> Option<VideoDeviceInfo> {
    let path = path.to_path_buf();
    let path_for_thread = path.clone();
    let (tx, rx) = mpsc::channel();

    std::thread::spawn(move || {
        let result = (|| -> Result<VideoDeviceInfo> {
            let device = VideoDevice::open_readonly(&path_for_thread)?;
            device.info()
        })();
        let _ = tx.send(result);
    });

    match rx.recv_timeout(timeout) {
        Ok(Ok(info)) => Some(info),
        Ok(Err(e)) => {
            debug!("Failed to get info for {:?}: {}", path, e);
            None
        }
        Err(mpsc::RecvTimeoutError::Timeout) => {
            warn!("Timed out probing video device: {:?}", path);
            None
        }
        Err(_) => None,
    }
}

fn sysfs_maybe_capture(path: &Path) -> bool {
    let name = match path.file_name().and_then(|n| n.to_str()) {
        Some(name) => name,
        None => return true,
    };
    let sysfs_base = Path::new("/sys/class/video4linux").join(name);

    let sysfs_name = read_sysfs_string(&sysfs_base.join("name"))
        .unwrap_or_default()
        .to_lowercase();
    let uevent = read_sysfs_string(&sysfs_base.join("device/uevent"))
        .unwrap_or_default()
        .to_lowercase();
    let driver = extract_uevent_value(&uevent, "driver");

    let mut maybe_capture = false;
    let capture_hints = [
        "capture",
        "hdmi",
        "usb",
        "uvc",
        "ms2109",
        "ms2130",
        "macrosilicon",
        "tc358743",
        "grabber",
    ];
    if capture_hints.iter().any(|hint| sysfs_name.contains(hint)) {
        maybe_capture = true;
    }
    if let Some(driver) = driver {
        if driver.contains("uvcvideo") || driver.contains("tc358743") {
            maybe_capture = true;
        }
    }

    let skip_hints = [
        "codec",
        "decoder",
        "encoder",
        "isp",
        "mem2mem",
        "m2m",
        "vbi",
        "radio",
        "metadata",
        "output",
    ];
    if skip_hints.iter().any(|hint| sysfs_name.contains(hint)) && !maybe_capture {
        return false;
    }

    true
}

fn read_sysfs_string(path: &Path) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .map(|value| value.trim().to_string())
}

fn extract_uevent_value(content: &str, key: &str) -> Option<String> {
    let key_upper = key.to_ascii_uppercase();
    for line in content.lines() {
        if let Some(value) = line.strip_prefix(&format!("{}=", key_upper)) {
            return Some(value.to_lowercase());
        }
    }
    None
}

/// Find the best video device for KVM use
pub fn find_best_device() -> Result<VideoDeviceInfo> {
    let devices = enumerate_devices()?;

    devices
        .into_iter()
        .next()
        .ok_or_else(|| AppError::VideoError("No video capture devices found".to_string()))
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
