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

use super::csi_bridge;
use super::format::{PixelFormat, Resolution};
use super::{is_rk_hdmirx_driver, is_rkcif_driver};
use crate::error::{AppError, Result};

/// Per-node probe limit; rkcif/RK628 ioctl chains can exceed 1s under contention.
const DEVICE_PROBE_TIMEOUT_MS: u64 = 10_000;

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
    /// Whether an HDMI signal is currently detected (CSI/HDMI bridge devices only;
    /// always `true` for USB capture cards).
    pub has_signal: bool,
    /// Path of the bridge subdev (`/dev/v4l-subdevN`) paired with this
    /// capture node, if any.  On Rockchip boards that wire an RK628 /
    /// TC358746 / RK-HDMIRX through `rkcif`, `QUERY_DV_TIMINGS`,
    /// `S_DV_TIMINGS`, `SUBSCRIBE_EVENT(SOURCE_CHANGE)`, `S_EDID` etc. all
    /// return `ENOTTY` on the video node — they only work here.  `None`
    /// for USB UVC and for bridges that expose DV ioctls on the video node
    /// directly (tc358743 via `uvcvideo`).
    pub subdev_path: Option<PathBuf>,
    /// Classification of the paired bridge (drives fingerprint logic for
    /// RK628's synthetic-VGA no-signal pattern).
    pub bridge_kind: Option<String>,
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

        // For CSI/HDMI bridges, try to locate the paired subdev *before*
        // the signal check: RK628 + rkcif places QUERY_DV_TIMINGS on the
        // subdev (the video node returns ENOTTY).  Tc358743 and rk_hdmirx
        // typically expose DV ioctls on the video node itself, but having
        // the subdev handle for EDID/event subscription doesn't hurt.
        let (subdev_path, bridge_kind) = if is_rkcif_driver(&caps.driver)
            || is_rk_hdmirx_driver(&caps.driver, &caps.card)
        {
            match csi_bridge::discover_subdev_for_video(&self.path) {
                Some((path, kind)) => (Some(path), Some(format!("{:?}", kind).to_lowercase())),
                None => (None, None),
            }
        } else {
            (None, None)
        };

        // Probe the HDMI source for both signal presence *and* the live
        // frame-rate.  rkcif's `VIDIOC_ENUM_FRAMEINTERVALS` returns a
        // meaningless `1.0..30.0` StepWise range, so the only trustworthy
        // fps for rkcif + RK628 / rk_hdmirx boards comes from the bridge
        // subdev's DV timings (pixelclock / total_width / total_height).
        //
        // Preference order:
        //   1. Bridge subdev — on rkcif boards this is the *only* node
        //      where QUERY_DV_TIMINGS works, and it lets the RK628
        //      fingerprint filter kick in before we return has_signal=true.
        //   2. Video node fallback — for rk_hdmirx / tc358743 where DV
        //      timings are exposed on the capture node directly.
        //   3. USB UVC — always true (no signal concept), no hdmi_fps.
        // Subdev-reported HDMI source mode (width, height, fps).  On rkcif +
        // RK628 boards this is the *only* place DV timings work; the video
        // node itself returns ENOTTY for QUERY/G_DV_TIMINGS, so without
        // threading this through to `enumerate_bridge_formats` the format
        // list ends up with zero resolutions and `select_resolution` falls
        // back to the user's preferred value (e.g. 4K) even when the real
        // source is 1080p.
        let mut subdev_hdmi_mode: Option<(u32, u32, Option<f64>)> = None;

        let (has_signal, hdmi_fps) = if let Some(subdev_path) = subdev_path.as_ref() {
            match csi_bridge::open_subdev(subdev_path) {
                Ok(subdev_fd) => {
                    let kind = parse_bridge_kind(bridge_kind.as_deref())
                        .unwrap_or(csi_bridge::CsiBridgeKind::Unknown);
                    let probe = csi_bridge::probe_signal(&subdev_fd, kind);
                    debug!(
                        "has_signal via subdev {:?} ({:?}): {:?}",
                        subdev_path, kind, probe
                    );
                    let fps = match &probe {
                        csi_bridge::ProbeResult::Locked(mode) => {
                            subdev_hdmi_mode = Some((mode.width, mode.height, mode.fps));
                            mode.fps
                        }
                        _ => None,
                    };
                    (probe.is_locked(), fps)
                }
                Err(e) => {
                    warn!("Failed to open subdev {:?}: {}", subdev_path, e);
                    (false, None)
                }
            }
        } else if is_rk_hdmirx_driver(&caps.driver, &caps.card)
            || is_rkcif_driver(&caps.driver)
        {
            let dv = self.current_dv_timings_mode();
            debug!(
                "has_signal via video node {:?} (driver={}): dv_timings={:?}",
                self.path, caps.driver, dv
            );
            let has_signal = dv
                .as_ref()
                .map(|(w, h, _)| *w > 64 && *h > 64)
                .unwrap_or(false);
            let fps = if has_signal {
                dv.and_then(|(_, _, f)| f)
            } else {
                None
            };
            (has_signal, fps)
        } else {
            (true, None)
        };

        let mut formats = if is_rk_hdmirx_driver(&caps.driver, &caps.card)
            || is_rkcif_driver(&caps.driver)
        {
            // CSI/HDMI bridge drivers (rk_hdmirx, rkcif) expose multiple pixel
            // formats via ENUM_FMT (e.g. rk_hdmirx: BGR3/NV24/NV16/NV12) but
            // `ENUM_FRAMESIZES` is fiction for these drivers (rkcif reports a
            // degenerate `64x64 StepWise 8/8` that only describes its DMA
            // engine, rk_hdmirx returns ENOTTY). The only authoritative
            // resolution is whatever the bridge subdev's DV timings report,
            // so we treat the HDMI source mode as the single allowed
            // resolution for every pixel format.
            self.enumerate_bridge_formats(subdev_hdmi_mode)?
        } else {
            self.enumerate_formats()?
        };

        // For CSI/HDMI bridges, the driver-enumerated fps list is fiction
        // (rkcif: always `1..30`; rk_hdmirx: typically `ENOTTY`).  Replace
        // it with the live HDMI source fps derived from the bridge DV
        // timings so the UI reflects what the sink is actually receiving.
        if let Some(fps) = hdmi_fps {
            override_resolution_fps(&mut formats, fps);
        }

        // Determine if this is likely an HDMI capture card
        let is_capture_card = Self::detect_capture_card(&caps.card, &caps.driver, &formats);

        // Calculate priority score
        let priority =
            Self::calculate_priority(&caps.card, &caps.driver, &formats, is_capture_card);

        debug!(
            "Device {:?}: {} formats, priority={}, has_signal={}, hdmi_fps={:?}, is_capture_card={}, subdev={:?}",
            self.path, formats.len(), priority, has_signal, hdmi_fps, is_capture_card, subdev_path
        );

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
            has_signal,
            subdev_path,
            bridge_kind,
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

    /// Enumerate formats for CSI/HDMI bridge devices (rk_hdmirx, rkcif).
    ///
    /// Uses `VIDIOC_ENUM_FMT` to discover all supported pixel formats (the
    /// output of `v4l2-ctl --list-formats`) and attaches the HDMI source
    /// resolution read from the bridge DV timings (or G_FMT as a last
    /// resort) as the single allowed resolution for every format.
    ///
    /// `ENUM_FRAMESIZES` is deliberately ignored here: rkcif advertises a
    /// degenerate `64x64 StepWise 8/8` that only describes its DMA engine
    /// (not what the HDMI source can actually deliver), and rk_hdmirx
    /// typically returns ENOTTY.  Neither the bridge nor rkcif performs
    /// any hardware scaling, so the capture resolution is always the
    /// HDMI source mode.
    ///
    /// Returned formats are sorted by `PixelFormat::priority()` so the
    /// higher-level `select_format` picks a sensible default (NV12 > YUYV on
    /// rkcif / rk_hdmirx) instead of whatever the driver happens to
    /// have stuck as the current active format.
    fn enumerate_bridge_formats(
        &self,
        subdev_hdmi_mode: Option<(u32, u32, Option<f64>)>,
    ) -> Result<Vec<FormatInfo>> {
        let queue = self.capture_queue_type()?;
        let current_fmt = self.get_format().ok();

        if let Some(fmt) = &current_fmt {
            debug!(
                "enumerate_bridge_formats: current G_FMT -> {:?} {}x{}",
                fmt.pixelformat, fmt.width, fmt.height
            );
        }

        // Preference order for the HDMI source resolution:
        //   1. Subdev-reported DV timings (authoritative on rkcif + RK628 where
        //      the video node returns ENOTTY for QUERY_DV_TIMINGS).
        //   2. Video-node DV timings / G_FMT (rk_hdmirx, tc358743 direct).
        let hdmi_mode = subdev_hdmi_mode
            .map(|(w, h, fps)| {
                let mut fps_list = Vec::new();
                if let Some(f) = fps {
                    fps_list.push(f);
                }
                if let Some(parm_fps) = self.current_parm_fps() {
                    fps_list.push(parm_fps);
                }
                normalize_fps_list(&mut fps_list);
                ResolutionInfo::new(w, h, fps_list)
            })
            .or_else(|| self.current_mode_resolution_info());
        if let Some(info) = &hdmi_mode {
            debug!(
                "enumerate_bridge_formats: HDMI source mode {}x{} (from {})",
                info.width,
                info.height,
                if subdev_hdmi_mode.is_some() {
                    "subdev"
                } else {
                    "video node"
                }
            );
        } else {
            debug!("enumerate_bridge_formats: no HDMI source mode available");
        }

        let mut formats: Vec<FormatInfo> = Vec::new();
        for desc in FormatIterator::new(&self.fd, queue) {
            let Some(format) = PixelFormat::from_v4l2r(desc.pixelformat) else {
                debug!(
                    "enumerate_bridge_formats: skipping unsupported fourcc {:?} ({})",
                    desc.pixelformat, desc.description
                );
                continue;
            };

            let resolutions = hdmi_mode.clone().into_iter().collect();

            formats.push(FormatInfo {
                format,
                resolutions,
                description: desc.description.clone(),
            });
        }

        if formats.is_empty() {
            // Fallback: driver refused ENUM_FMT entirely, use just the current
            // active format reported by G_FMT so we still have something.
            if let Some(fmt) = current_fmt {
                if let Some(format) = PixelFormat::from_v4l2r(fmt.pixelformat) {
                    let description = self
                        .format_description(fmt.pixelformat)
                        .unwrap_or_else(|| format.to_string());
                    let resolutions = hdmi_mode.into_iter().collect();
                    formats.push(FormatInfo {
                        format,
                        resolutions,
                        description,
                    });
                }
            }
        }

        // Highest priority first (MJPEG > NV12 > NV16 > NV24 > BGR24 > ...).
        formats.sort_by(|a, b| b.format.priority().cmp(&a.format.priority()));

        debug!(
            "enumerate_bridge_formats: resolved formats {:?}",
            formats
                .iter()
                .map(|f| format!("{}({} res)", f.format, f.resolutions.len()))
                .collect::<Vec<_>>()
        );

        Ok(formats)
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
                                // StepWise ranges are ignored on purpose: on
                                // CSI/HDMI bridge drivers (rkcif) the range
                                // only describes the DMA engine's capability
                                // and not what the HDMI source can deliver,
                                // so synthesising candidate resolutions from
                                // it is misleading. Bridge devices go
                                // through `enumerate_bridge_formats` and use
                                // the DV-timings source mode directly; for
                                // any other driver that emits StepWise we
                                // fall back to the current active mode below.
                                debug!(
                                    "ENUM_FRAMESIZES {:?}: ignoring StepWise {}x{} - {}x{} step {}/{}",
                                    fourcc, s.min_width, s.min_height,
                                    s.max_width, s.max_height,
                                    s.step_width, s.step_height
                                );
                                if resolutions.is_empty() {
                                    should_fallback_to_current_mode = true;
                                }
                                break;
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
            "rkcif",
            "rk_hdmirx",
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

    // First pass: collect candidates that pass the sysfs-based pre-filter.
    // This avoids opening orphan /dev/videoN nodes (ENODEV) and m2m codec
    // nodes (ENOTTY) that would otherwise waste one syscall + one ioctl each.
    let mut candidates: Vec<PathBuf> = Vec::new();
    for entry in std::fs::read_dir("/dev")
        .map_err(|e| AppError::VideoError(format!("Failed to read /dev: {}", e)))?
    {
        let Ok(entry) = entry else { continue };
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
        candidates.push(path);
    }

    collapse_rkcif_probe_candidates(&mut candidates);

    // Second pass: probe the remaining candidates in parallel. Each probe
    // already spawns its own worker thread inside `probe_device_with_timeout`,
    // so the total wall-clock time is bounded by `DEVICE_PROBE_TIMEOUT_MS`
    // rather than (N × per-probe-latency).
    let timeout = Duration::from_millis(DEVICE_PROBE_TIMEOUT_MS);
    let mut handles = Vec::with_capacity(candidates.len());
    for path in candidates {
        handles.push(std::thread::spawn(move || {
            (path.clone(), probe_device_with_timeout(&path, timeout))
        }));
    }

    let mut devices = Vec::new();
    for handle in handles {
        let (path, info) = match handle.join() {
            Ok(pair) => pair,
            Err(_) => continue,
        };
        match info {
            Some(info) => {
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

    // Sort by priority (highest first), then by path (lowest first) as tiebreaker.
    // The path tiebreaker ensures deterministic ordering when multiple sub-devices
    // share the same priority (e.g. rkcif nodes), so that /dev/video0 is preferred
    // over /dev/video10 after deduplication.
    devices.sort_by(|a, b| b.priority.cmp(&a.priority).then_with(|| a.path.cmp(&b.path)));

    // Deduplicate rkcif sub-devices: the driver exposes many /dev/video* nodes
    // for a single MIPI CSI pipeline. Keep only the highest-priority node per
    // (driver, bus_info) group so users see one device instead of ~11.
    dedup_platform_subdevices(&mut devices);

    info!("Found {} video capture devices", devices.len());
    Ok(devices)
}

/// Collapse platform sub-device nodes that share the same driver + bus_info
/// into a single entry (the one with the highest priority / most formats).
/// Currently applies to the `rkcif` driver on Rockchip SoCs where each
/// media-pipeline link creates its own `/dev/video*` node.
fn dedup_platform_subdevices(devices: &mut Vec<VideoDeviceInfo>) {
    // devices is already sorted by priority (descending).
    // Walk the list and keep only the first (highest-priority) representative
    // of each (driver, bus_info) group that needs deduplication.
    let mut seen = std::collections::HashSet::new();
    devices.retain(|d| {
        if !is_rkcif_driver(&d.driver) || d.bus_info.is_empty() {
            return true;
        }
        let key = (d.driver.clone(), d.bus_info.clone());
        seen.insert(key)
    });
}

/// rkcif registers many `/dev/video*` queues; probing all in parallel can
/// contend and time out. Keep one node per board (lowest `videoN`).
fn collapse_rkcif_probe_candidates(candidates: &mut Vec<PathBuf>) {
    let mut rkcif: Vec<PathBuf> = Vec::new();
    let mut rest: Vec<PathBuf> = Vec::new();
    for p in candidates.drain(..) {
        if sysfs_uevent_driver(&p).is_some_and(|d| d.contains("rkcif")) {
            rkcif.push(p);
        } else {
            rest.push(p);
        }
    }
    if let Some(one) = rkcif
        .iter()
        .min_by_key(|p| video_index(p).unwrap_or(u32::MAX))
        .cloned()
    {
        rest.push(one);
    }
    *candidates = rest;
}

fn sysfs_uevent_driver(path: &Path) -> Option<String> {
    let name = path.file_name()?.to_str()?;
    let uevent =
        read_sysfs_string(&Path::new("/sys/class/video4linux").join(name).join("device/uevent"))?;
    extract_uevent_value(&uevent, "driver")
}

fn video_index(path: &Path) -> Option<u32> {
    path.file_name()?
        .to_str()?
        .strip_prefix("video")?
        .parse()
        .ok()
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

    // Fast-path: nodes whose filename clearly marks them as m2m codecs
    // (e.g. /dev/video-enc0, /dev/video-dec0 on Rockchip). These never
    // answer VIDIOC_QUERYCAP as capture devices.
    let name_lower = name.to_ascii_lowercase();
    let filename_skip = ["-enc", "-dec", "-codec", "-m2m", "-vepu", "-vdpu"];
    if filename_skip.iter().any(|hint| name_lower.contains(hint)) {
        return false;
    }

    let sysfs_base = Path::new("/sys/class/video4linux").join(name);

    // Orphan /dev/videoN nodes (no matching sysfs entry) can appear when the
    // kernel driver that created them has been unloaded but the device nodes
    // were never cleaned up. Opening them returns ENODEV; skip the probe.
    if !sysfs_base.exists() {
        debug!("Skipping {:?}: no matching /sys/class/video4linux entry", path);
        return false;
    }

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
        "rkcif",
        "rk_hdmirx",
    ];
    if capture_hints.iter().any(|hint| sysfs_name.contains(hint)) {
        maybe_capture = true;
    }
    if let Some(driver) = &driver {
        if driver.contains("uvcvideo")
            || driver.contains("tc358743")
            || driver.contains("rkcif")
            || driver.contains("rk_hdmirx")
        {
            maybe_capture = true;
        }
    }

    // Skip known non-capture drivers (RK video codecs, Hantro VPU, ISP/VPE
    // pipelines, MIPI ISP statistics / params nodes). These would otherwise
    // succeed QUERYCAP but expose only VIDEO_M2M / STATS / PARAMS and get
    // filtered later — skipping here saves an open() + ioctl() per node.
    let driver_skip = [
        "rkvenc", "rkvdec", "vepu", "vdpu", "hantro", "mpp_", "rockchip-vpu",
    ];
    if let Some(driver) = &driver {
        if driver_skip.iter().any(|hint| driver.contains(hint)) {
            return false;
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
        // rkisp sub-nodes that are not video capture queues
        "rkisp-statistics",
        "rkisp-input-params",
        "rkisp_rawrd",
        "rkisp_rawwr",
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

/// Parse the `bridge_kind` string serialised into `VideoDeviceInfo` back
/// into the strongly-typed enum used by [`csi_bridge`].
pub(crate) fn parse_bridge_kind(kind: Option<&str>) -> Option<csi_bridge::CsiBridgeKind> {
    Some(match kind? {
        "rk628" => csi_bridge::CsiBridgeKind::Rk628,
        "rkhdmirx" => csi_bridge::CsiBridgeKind::RkHdmirx,
        "tc358743" => csi_bridge::CsiBridgeKind::Tc358743,
        "unknown" => csi_bridge::CsiBridgeKind::Unknown,
        _ => return None,
    })
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

/// Replace every `ResolutionInfo::fps` in `formats` with the single HDMI
/// source frame-rate.  Used for CSI/HDMI bridge devices (rkcif, rk_hdmirx)
/// whose `VIDIOC_ENUM_FRAMEINTERVALS` returns meaningless StepWise values
/// — the only trustworthy fps comes from the bridge DV-timings on the
/// paired subdev.  Silently no-op when `fps` normalises to empty.
fn override_resolution_fps(formats: &mut [FormatInfo], fps: f64) {
    let mut normalized = vec![fps];
    normalize_fps_list(&mut normalized);
    if normalized.is_empty() {
        return;
    }
    for fi in formats.iter_mut() {
        for res in fi.resolutions.iter_mut() {
            res.fps = normalized.clone();
        }
    }
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
