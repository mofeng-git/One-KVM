//! V4L2 device enumeration and capability query

use serde::{Deserialize, Serialize};
use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;
use tracing::{debug, info, warn};
use v4l2r::bindings::{
    v4l2_bt_timings, v4l2_dv_timings, v4l2_frmivalenum, v4l2_frmsizeenum, v4l2_streamparm,
    V4L2_DV_BT_656_1120,
};
use v4l2r::ioctl::{
    self, Capabilities, Capability as V4l2rCapability, FormatIterator, FrmIvalTypes, FrmSizeTypes,
};
use v4l2r::nix::errno::Errno;
use v4l2r::{Format as V4l2rFormat, QueueType};

use super::format::{PixelFormat, Resolution};
use super::is_rk_hdmirx_driver;
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
            .map_err(|e| {
                AppError::VideoError(format!("Failed to open device {:?}: {}", path, e))
            })?;

        Ok(Self { path, fd })
    }

    /// Open a video device read-only (for probing/enumeration)
    pub fn open_readonly(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        debug!("Opening video device (read-only): {:?}", path);

        let fd = File::options().read(true).open(&path).map_err(|e| {
            AppError::VideoError(format!("Failed to open device {:?}: {}", path, e))
        })?;

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

        let formats = if is_rk_hdmirx_driver(&caps.driver, &caps.card) {
            self.enumerate_current_format_only()?
        } else {
            self.enumerate_formats()?
        };

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
                let is_current_format = self.current_active_format() == Some(format);

                if resolutions.is_empty() && !is_current_format {
                    debug!(
                        "Skipping format {:?} ({}): not usable for current active mode",
                        desc.pixelformat, desc.description
                    );
                    continue;
                }

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

    fn enumerate_current_format_only(&self) -> Result<Vec<FormatInfo>> {
        let current = self.get_format()?;
        let Some(format) = PixelFormat::from_v4l2r(current.pixelformat) else {
            debug!(
                "Current active format {:?} is not supported by One-KVM, falling back to full enumeration",
                current.pixelformat
            );
            return self.enumerate_formats();
        };

        let description = self
            .format_description(current.pixelformat)
            .unwrap_or_else(|| format.to_string());

        let mut resolutions = self.enumerate_resolutions(current.pixelformat)?;
        if resolutions.is_empty() {
            if let Some(current_mode) = self.current_mode_resolution_info() {
                resolutions.push(current_mode);
            }
        }

        Ok(vec![FormatInfo {
            format,
            resolutions,
            description,
        }])
    }

    /// Enumerate resolutions for a specific format
    fn enumerate_resolutions(&self, fourcc: v4l2r::PixelFormat) -> Result<Vec<ResolutionInfo>> {
        let mut resolutions = Vec::new();
        let mut should_fallback_to_current_mode = false;

        let mut index = 0u32;
        loop {
            match ioctl::enum_frame_sizes::<v4l2_frmsizeenum>(&self.fd, index, fourcc) {
                Ok(size) => {
                    if let Some(size) = size.size() {
                        match size {
                            FrmSizeTypes::Discrete(d) => {
                                let fps = self
                                    .enumerate_fps(fourcc, d.width, d.height)
                                    .unwrap_or_default();
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
                                        resolutions
                                            .push(ResolutionInfo::new(res.width, res.height, fps));
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
                    let is_unsupported = matches!(
                        e,
                        v4l2r::ioctl::FrameSizeError::IoctlError(err)
                            if matches!(err, Errno::ENOTTY | Errno::ENOSYS | Errno::EOPNOTSUPP)
                    );
                    if is_unsupported && resolutions.is_empty() {
                        should_fallback_to_current_mode = true;
                    }
                    if !is_einval && !is_unsupported {
                        debug!("Failed to enumerate frame sizes for {:?}: {}", fourcc, e);
                    }
                    break;
                }
            }
        }

        if should_fallback_to_current_mode {
            if let Some(resolution) = self.current_mode_resolution_info() {
                if self.format_works_for_resolution(fourcc, resolution.width, resolution.height) {
                    debug!(
                        "Falling back to current active mode for {:?}: {}x{} @ {:?} fps",
                        fourcc, resolution.width, resolution.height, resolution.fps
                    );
                    resolutions.push(resolution);
                } else {
                    debug!(
                        "Skipping current-mode fallback for {:?}: TRY_FMT rejected {}x{}",
                        fourcc, resolution.width, resolution.height
                    );
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
    ) -> Result<Vec<f64>> {
        let mut fps_list = Vec::new();
        let mut should_fallback_to_current_mode = false;

        let mut index = 0u32;
        loop {
            match ioctl::enum_frame_intervals::<v4l2_frmivalenum>(
                &self.fd, index, fourcc, width, height,
            ) {
                Ok(interval) => {
                    if let Some(interval) = interval.intervals() {
                        match interval {
                            FrmIvalTypes::Discrete(fraction) => {
                                if fraction.numerator > 0 && fraction.denominator > 0 {
                                    let fps =
                                        fraction.denominator as f64 / fraction.numerator as f64;
                                    fps_list.push(fps);
                                }
                            }
                            FrmIvalTypes::StepWise(step) => {
                                if step.max.numerator > 0 && step.max.denominator > 0 {
                                    let min_fps =
                                        step.max.denominator as f64 / step.max.numerator as f64;
                                    let max_fps =
                                        step.min.denominator as f64 / step.min.numerator as f64;
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
                    let is_unsupported = matches!(
                        e,
                        v4l2r::ioctl::FrameIntervalsError::IoctlError(err)
                            if matches!(err, Errno::ENOTTY | Errno::ENOSYS | Errno::EOPNOTSUPP)
                    );
                    if is_unsupported && fps_list.is_empty() {
                        should_fallback_to_current_mode = true;
                    }
                    if !is_einval && !is_unsupported {
                        debug!(
                            "Failed to enumerate frame intervals for {:?} {}x{}: {}",
                            fourcc, width, height, e
                        );
                    }
                    break;
                }
            }
        }

        if should_fallback_to_current_mode {
            fps_list.extend(self.current_mode_fps());
        }

        normalize_fps_list(&mut fps_list);
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

        priority += max_resolution / 100000;

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

    fn current_mode_resolution_info(&self) -> Option<ResolutionInfo> {
        let (width, height) = self
            .current_dv_timings_mode()
            .map(|(width, height, _)| (width, height))
            .or_else(|| self.current_format_resolution())?;
        Some(ResolutionInfo::new(width, height, self.current_mode_fps()))
    }

    fn current_mode_fps(&self) -> Vec<f64> {
        let mut fps = Vec::new();

        if let Some(frame_rate) = self.current_parm_fps() {
            fps.push(frame_rate);
        }

        if let Some((_, _, Some(frame_rate))) = self.current_dv_timings_mode() {
            fps.push(frame_rate);
        }

        normalize_fps_list(&mut fps);
        fps
    }

    fn current_parm_fps(&self) -> Option<f64> {
        let queue = self.capture_queue_type().ok()?;
        let params: v4l2_streamparm = ioctl::g_parm(&self.fd, queue).ok()?;
        let capture = unsafe { params.parm.capture };
        let timeperframe = capture.timeperframe;
        if timeperframe.numerator == 0 || timeperframe.denominator == 0 {
            return None;
        }
        Some(timeperframe.denominator as f64 / timeperframe.numerator as f64)
    }

    fn current_dv_timings_mode(&self) -> Option<(u32, u32, Option<f64>)> {
        let timings = ioctl::query_dv_timings::<v4l2_dv_timings>(&self.fd)
            .or_else(|_| ioctl::g_dv_timings::<v4l2_dv_timings>(&self.fd))
            .ok()?;

        if timings.type_ != V4L2_DV_BT_656_1120 {
            return None;
        }

        let bt = unsafe { timings.__bindgen_anon_1.bt };
        if bt.width == 0 || bt.height == 0 {
            return None;
        }

        Some((bt.width, bt.height, dv_timings_fps(&bt)))
    }

    /// Query current DV timings resolution for runtime change detection.
    ///
    /// Returns the active resolution reported by DV timings (used by CSI/HDMI bridges
    /// such as TC358743, rk_hdmirx, etc.).  Returns `None` when the device does not
    /// support DV timings or no signal is detected.
    pub fn query_dv_timings_resolution(&self) -> Option<Resolution> {
        let (w, h, _fps) = self.current_dv_timings_mode()?;
        Some(Resolution::new(w, h))
    }

    fn current_format_resolution(&self) -> Option<(u32, u32)> {
        let format = self.get_format().ok()?;
        if format.width == 0 || format.height == 0 {
            return None;
        }
        Some((format.width, format.height))
    }

    fn current_active_format(&self) -> Option<PixelFormat> {
        let format = self.get_format().ok()?;
        PixelFormat::from_v4l2r(format.pixelformat)
    }

    fn format_description(&self, fourcc: v4l2r::PixelFormat) -> Option<String> {
        let queue = self.capture_queue_type().ok()?;
        FormatIterator::new(&self.fd, queue)
            .find(|desc| desc.pixelformat == fourcc)
            .map(|desc| desc.description)
    }

    fn format_works_for_resolution(
        &self,
        fourcc: v4l2r::PixelFormat,
        width: u32,
        height: u32,
    ) -> bool {
        let queue = match self.capture_queue_type() {
            Ok(queue) => queue,
            Err(_) => return false,
        };

        let mut fmt = match ioctl::g_fmt::<V4l2rFormat>(&self.fd, queue) {
            Ok(fmt) => fmt,
            Err(_) => return false,
        };

        fmt.width = width;
        fmt.height = height;
        fmt.pixelformat = fourcc;

        let actual = match ioctl::try_fmt::<_, V4l2rFormat>(&self.fd, (queue, &fmt)) {
            Ok(actual) => actual,
            Err(_) => return false,
        };

        actual.pixelformat == fourcc && actual.width == width && actual.height == height
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
        "codec", "decoder", "encoder", "isp", "mem2mem", "m2m", "vbi", "radio", "metadata",
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

fn dv_timings_fps(bt: &v4l2_bt_timings) -> Option<f64> {
    let total_width = bt.width + bt.hfrontporch + bt.hsync + bt.hbackporch;
    let total_height = if bt.interlaced != 0 {
        bt.height
            + bt.vfrontporch
            + bt.vsync
            + bt.vbackporch
            + bt.il_vfrontporch
            + bt.il_vsync
            + bt.il_vbackporch
    } else {
        bt.height + bt.vfrontporch + bt.vsync + bt.vbackporch
    };

    if bt.pixelclock == 0 || total_width == 0 || total_height == 0 {
        return None;
    }

    Some(bt.pixelclock as f64 / total_width as f64 / total_height as f64)
}

fn normalize_fps_list(fps_list: &mut Vec<f64>) {
    fps_list.retain(|fps| fps.is_finite() && *fps > 0.0);
    for fps in fps_list.iter_mut() {
        *fps = (*fps * 100.0).round() / 100.0;
    }
    fps_list.sort_by(|a, b| b.total_cmp(a));
    fps_list.dedup_by(|a, b| (*a - *b).abs() < 0.01);
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
