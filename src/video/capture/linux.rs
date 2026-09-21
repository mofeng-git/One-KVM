//! V4L2 capture implementation using v4l2r (ioctl layer).

use std::fs::File;
use std::io;
use std::os::fd::AsFd;
#[cfg(any(target_arch = "aarch64", target_arch = "arm"))]
use std::os::fd::OwnedFd;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use nix::poll::{poll, PollFd, PollFlags, PollTimeout};
use tracing::{debug, info, warn};
use v4l2r::bindings::{
    v4l2_dv_timings, v4l2_requestbuffers, v4l2_streamparm, v4l2_streamparm__bindgen_ty_1,
    V4L2_DV_BT_656_1120,
};
use v4l2r::ioctl::{
    self, Capabilities, Capability as V4l2rCapability, EventType, IntoErrno, MemoryConsistency,
    PlaneMapping, QBufPlane, QBuffer, QueryBuffer, QueryDvTimingsError, SubscribeEventFlags,
    V4l2Buffer,
};
use v4l2r::memory::{MemoryType, MmapHandle};
use v4l2r::nix::errno::Errno;
use v4l2r::{Format as V4l2rFormat, PixelFormat as V4l2rPixelFormat, QueueType};

use super::CaptureReadError;
use crate::error::{AppError, Result};
use crate::video::device::bridge::{self as csi_bridge, CsiBridgeKind, ProbeResult};
use crate::video::device::VideoControlMode;
use crate::video::format::{PixelFormat, Resolution};
use crate::video::signal::SignalStatus;

#[cfg(any(test, target_arch = "aarch64", target_arch = "arm"))]
#[path = "dmabuf_layout.rs"]
mod dmabuf_layout;

/// Metadata for a captured frame.
#[derive(Debug, Clone, Copy)]
pub struct CaptureMeta {
    pub bytes_used: usize,
    pub sequence: u64,
}

/// When set, DV ioctls use the subdev (rkcif: video node has no DV ioctls).
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
        self.subdev_path.is_some()
    }
}

/// V4L2 capture stream backed by v4l2r ioctl.
pub struct CaptureStream {
    fd: File,
    queue: QueueType,
    resolution: Resolution,
    format: PixelFormat,
    source_fps: Option<f64>,
    stride: u32,
    timeout: Duration,
    mappings: Vec<Vec<PlaneMapping>>,
    subdev_fd: Option<File>,
    bridge_kind: Option<CsiBridgeKind>,
    native_hdmirx_state: Option<NativeHdmirxState>,
    native_hdmirx_next_state_check: Option<Instant>,
    #[cfg(any(target_arch = "aarch64", target_arch = "arm"))]
    dma_layout_bytes: Option<usize>,
}

fn open_capture_device(path: &Path) -> io::Result<File> {
    File::options()
        .read(true)
        .write(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(path)
}

impl CaptureStream {
    /// UVC: uses `resolution`. CSI bridges: DV-probe first; may return `CaptureNoSignal`.
    pub fn open(
        device_path: impl AsRef<Path>,
        resolution: Resolution,
        format: PixelFormat,
        fps: u32,
        buffer_count: u32,
        timeout: Duration,
    ) -> Result<Self> {
        Self::open_with_bridge(
            device_path,
            resolution,
            format,
            fps,
            buffer_count,
            timeout,
            BridgeContext::default(),
            VideoControlMode::Configurable,
        )
    }

    /// With subdev: probe DV on subdev before opening video (RK628 safety); may ignore requested size.
    pub fn open_with_bridge(
        device_path: impl AsRef<Path>,
        resolution: Resolution,
        format: PixelFormat,
        fps: u32,
        buffer_count: u32,
        timeout: Duration,
        bridge: BridgeContext,
        control_mode: VideoControlMode,
    ) -> Result<Self> {
        // Probe subdev before video open (RK628: no-signal must not reach capture STREAMON).
        let mut subdev_fd_opt: Option<File> = None;
        let mut subdev_dv_mode: Option<csi_bridge::DvTimingsMode> = None;

        if let Some(subdev_path) = bridge.subdev_path.as_ref() {
            let subdev_fd = csi_bridge::open_subdev(subdev_path).map_err(|e| {
                AppError::VideoError(format!(
                    "Failed to open CSI bridge subdev {:?}: {}",
                    subdev_path, e
                ))
            })?;

            let kind = bridge.kind.unwrap_or(CsiBridgeKind::Unknown);
            match csi_bridge::probe_signal(&subdev_fd, kind) {
                ProbeResult::Locked(mode) => {
                    info!(
                        "Subdev {:?} locked: {}x{} @ {}Hz",
                        subdev_path, mode.width, mode.height, mode.pixelclock
                    );
                    csi_bridge::apply_dv_timings(&subdev_fd, mode.raw);
                    if let Err(e) = csi_bridge::subscribe_source_change(&subdev_fd) {
                        debug!("subdev SOURCE_CHANGE subscribe failed: {}", e);
                    }
                    subdev_dv_mode = Some(mode);
                }
                other => {
                    let status = other.as_status().unwrap_or(SignalStatus::NoSignal);
                    debug!(
                        "Subdev {:?} reports no signal ({:?}) — refusing STREAMON",
                        subdev_path, status
                    );
                    return Err(AppError::CaptureNoSignal {
                        kind: status.as_str().to_string(),
                    });
                }
            }
            subdev_fd_opt = Some(subdev_fd);
        }

        // ── Phase 1: open the capture (video) node ─────────────────────
        let mut fd = open_capture_device(device_path.as_ref())
            .map_err(|e| AppError::VideoError(format!("Failed to open device: {}", e)))?;

        let caps: V4l2rCapability = ioctl::querycap(&fd)
            .map_err(|e| AppError::VideoError(format!("Failed to query capabilities: {}", e)))?;
        let caps_flags = caps.device_caps();
        let is_source_following = control_mode == VideoControlMode::SourceFollowing;
        let is_native_hdmirx = bridge.kind == Some(CsiBridgeKind::RkHdmirx);

        // Prefer multi-planar capture when available, as it is required for some
        // devices/pixel formats (e.g. NV12 via VIDEO_CAPTURE_MPLANE).
        let queue = if caps_flags.contains(Capabilities::VIDEO_CAPTURE_MPLANE) {
            QueueType::VideoCaptureMplane
        } else if caps_flags.contains(Capabilities::VIDEO_CAPTURE) {
            QueueType::VideoCapture
        } else {
            return Err(AppError::VideoError(
                "Device does not support capture queues".to_string(),
            ));
        };

        // CSI/HDMI bridge without a subdev pairing (tc358743 on uvcvideo,
        // rk_hdmirx on RK3588): probe DV timings on the video node, with
        // the same no-signal gate as the subdev path.  When we *do* have
        // a subdev, reuse its already-probed mode to drive S_FMT.
        let dv_mode = if let Some(mode) = subdev_dv_mode.as_ref() {
            Some(DvTimingsMode {
                width: mode.width,
                height: mode.height,
                fps: mode.fps,
                signature: None,
            })
        } else if is_source_following {
            // The native RK3588 HDMI RX driver already latches detected
            // timings while locking the input.  S_DV_TIMINGS is unnecessary
            // there and rejects some otherwise valid sources whose measured
            // porches do not exactly match its CEA table.
            Some(probe_dv_timings(&fd, !is_native_hdmirx)?)
        } else {
            None
        };

        // rkcif + RK628: G_FMT is often 0×0 until the first S_FMT; G_FMT may
        // also fail. With DV timings from the subdev, build the format (same as
        // `v4l2-ctl --set-fmt-video=width=…,height=…`).
        let mut fmt: V4l2rFormat = match (
            ioctl::g_fmt::<V4l2rFormat>(&fd, queue),
            is_source_following,
            dv_mode.as_ref(),
        ) {
            (Ok(f), _, _) if f.width > 0 && f.height > 0 => f,
            (_, true, Some(m)) => {
                let fourcc = format.to_fourcc();
                V4l2rFormat::from((&fourcc, (m.width as usize, m.height as usize)))
            }
            (Ok(f), _, _) => f,
            (Err(e), _, _) => {
                return Err(AppError::VideoError(format!(
                    "Failed to get device format: {}",
                    e
                )));
            }
        };

        // Prefer the DV-timings-reported geometry for CSI bridges — the
        // source, not the user config, dictates what the capture hardware
        // will actually deliver.
        let (target_w, target_h) = match dv_mode.as_ref() {
            Some(DvTimingsMode { width, height, .. }) => (*width, *height),
            None => (resolution.width, resolution.height),
        };
        fmt.width = target_w;
        fmt.height = target_h;
        let requested_fourcc = V4l2rPixelFormat::from(&format.to_fourcc());
        if is_native_hdmirx && fmt.pixelformat != requested_fourcc {
            // rk_hdmirx exposes all possible HDMI input encodings through
            // ENUM_FMT but can capture only the encoding currently present on
            // the wire.  Follow G_FMT so a source-side RGB/YUV transition can
            // recover even if the saved configuration still names the old
            // FourCC.  The negotiated format is returned to the caller, which
            // rebuilds the encoder when it changed.
            info!(
                "rk_hdmirx input format changed/requested {:?}, following active {:?}",
                requested_fourcc, fmt.pixelformat
            );
        } else {
            fmt.pixelformat = requested_fourcc;
        }

        let actual_fmt: V4l2rFormat = ioctl::s_fmt(&mut fd, (queue, &fmt))
            .map_err(|e| AppError::VideoError(format!("Failed to set device format: {}", e)))?;

        let actual_resolution = Resolution::new(actual_fmt.width, actual_fmt.height);
        let actual_format = match PixelFormat::from_v4l2r(actual_fmt.pixelformat) {
            Some(format) => format,
            None if is_native_hdmirx => {
                return Err(AppError::VideoError(format!(
                    "Native HDMI RX input format {:?} is not supported; configure the HDMI source for 8-bit RGB/YUV output",
                    actual_fmt.pixelformat
                )));
            }
            None => format,
        };

        let native_hdmirx_state = is_native_hdmirx.then(|| NativeHdmirxState {
            width: actual_fmt.width,
            height: actual_fmt.height,
            pixelformat: actual_fmt.pixelformat,
            timings: dv_mode.as_ref().and_then(|mode| mode.signature),
        });
        let native_hdmirx_next_state_check =
            native_hdmirx_state.map(|_| Instant::now() + Duration::from_secs(1));

        let stride = actual_fmt
            .plane_fmt
            .first()
            .map(|p| p.bytesperline)
            .unwrap_or_else(|| match actual_format.bytes_per_pixel() {
                Some(bpp) => actual_resolution.width * bpp as u32,
                None => actual_resolution.width,
            });

        if fps > 0 {
            match set_fps(&fd, queue, fps) {
                Ok(()) => {}
                Err(ioctl::GParmError::IoctlError(err))
                    if matches!(err, Errno::ENOTTY | Errno::ENOSYS | Errno::EOPNOTSUPP) => {}
                Err(e) => warn!("Failed to set hardware FPS: {}", e),
            }
        }

        let req: v4l2_requestbuffers = ioctl::reqbufs(
            &fd,
            queue,
            MemoryType::Mmap,
            buffer_count,
            MemoryConsistency::empty(),
        )
        .map_err(|e| AppError::VideoError(format!("Failed to request buffers: {}", e)))?;
        let allocated = req.count as usize;
        if allocated == 0 {
            return Err(AppError::VideoError(
                "Driver returned zero capture buffers".to_string(),
            ));
        }

        let mut mappings = Vec::with_capacity(allocated);
        for index in 0..allocated as u32 {
            let query: QueryBuffer = ioctl::querybuf(&fd, queue, index as usize).map_err(|e| {
                AppError::VideoError(format!("Failed to query buffer {}: {}", index, e))
            })?;

            if query.planes.is_empty() {
                return Err(AppError::VideoError(format!(
                    "Driver returned zero planes for buffer {}",
                    index
                )));
            }

            let mut plane_maps = Vec::with_capacity(query.planes.len());
            for plane in &query.planes {
                let mapping = ioctl::mmap(&fd, plane.mem_offset, plane.length).map_err(|e| {
                    AppError::VideoError(format!("Failed to mmap buffer {}: {}", index, e))
                })?;
                plane_maps.push(mapping);
            }
            mappings.push(plane_maps);
        }

        #[cfg(any(target_arch = "aarch64", target_arch = "arm"))]
        let dma_layout_bytes = PixelFormat::from_v4l2r(actual_fmt.pixelformat).and_then(|format| {
            dmabuf_layout::DmaCaptureLayout {
                native_hdmi: is_native_hdmirx,
                driver: &caps.driver,
                bus_info: &caps.bus_info,
                configurable_usb: !is_source_following
                    && bridge.kind.is_none()
                    && !bridge.has_subdev(),
                single_planar: queue == QueueType::VideoCapture,
                fourcc: format.to_fourcc(),
                width: actual_resolution.width,
                height: actual_resolution.height,
                stride,
            }
            .minimum_bytes()
        });

        let mut stream = Self {
            fd,
            queue,
            resolution: actual_resolution,
            format: actual_format,
            source_fps: dv_mode.as_ref().and_then(|mode| mode.fps),
            stride,
            timeout,
            mappings,
            subdev_fd: subdev_fd_opt,
            bridge_kind: bridge.kind,
            native_hdmirx_state,
            native_hdmirx_next_state_check,
            #[cfg(any(target_arch = "aarch64", target_arch = "arm"))]
            dma_layout_bytes,
        };

        stream.queue_all_buffers()?;
        ioctl::streamon(&stream.fd, stream.queue)
            .map_err(|e| AppError::VideoError(format!("Failed to start capture stream: {}", e)))?;

        // When the subdev path was used, SOURCE_CHANGE was already
        // subscribed *there* (the rkcif video node returns ENOTTY).
        // Otherwise try on the video node as a best-effort fallback for
        // drivers that do honour it (tc358743/uvcvideo, rk_hdmirx).
        if stream.subdev_fd.is_none() {
            match ioctl::subscribe_event(
                &stream.fd,
                EventType::SourceChange(0),
                SubscribeEventFlags::empty(),
            ) {
                Ok(()) => debug!("Subscribed to V4L2_EVENT_SOURCE_CHANGE on video node"),
                Err(e) => debug!(
                    "V4L2_EVENT_SOURCE_CHANGE subscription unavailable on video node \
                     ({}), falling back to timeout-based restart",
                    e
                ),
            }
        }

        Ok(stream)
    }

    pub fn resolution(&self) -> Resolution {
        self.resolution
    }

    pub fn format(&self) -> PixelFormat {
        self.format
    }

    pub fn source_fps(&self) -> Option<f64> {
        self.source_fps
    }

    pub fn stride(&self) -> u32 {
        self.stride
    }

    /// Re-probe DV timings on the persistent subdev handle (no extra `open`).
    pub fn probe_bridge_signal(&self) -> Option<ProbeResult> {
        let subdev_fd = self.subdev_fd.as_ref()?;
        Some(csi_bridge::probe_signal(
            subdev_fd,
            self.bridge_kind.unwrap_or(CsiBridgeKind::Unknown),
        ))
    }

    /// Like [`Self::probe_bridge_signal`] but isolates the ioctl on a dup'd
    /// fd with a wall-clock cap — see [`csi_bridge::probe_signal_thread_timeout`].
    pub fn probe_bridge_signal_with_timeout(&self, limit: Duration) -> Option<ProbeResult> {
        let subdev_fd = self.subdev_fd.as_ref()?;
        csi_bridge::probe_signal_thread_timeout(
            subdev_fd,
            self.bridge_kind.unwrap_or(CsiBridgeKind::Unknown),
            limit,
        )
    }

    fn expected_capture_bytes(&self) -> Option<usize> {
        if self.format.is_compressed() {
            return None;
        }
        // Stride is bytesperline; packed formats use stride × height (not × bpp).
        if self.format.bytes_per_pixel().is_some() {
            return (self.stride as usize).checked_mul(self.resolution.height as usize);
        }
        match self.format {
            PixelFormat::Nv12 | PixelFormat::Nv21 | PixelFormat::Yuv420 | PixelFormat::Yvu420 => {
                (self.stride as usize)
                    .checked_mul(self.resolution.height as usize)?
                    .checked_mul(3)?
                    .checked_div(2)
            }
            PixelFormat::Nv16 => (self.stride as usize)
                .checked_mul(self.resolution.height as usize)?
                .checked_mul(2),
            PixelFormat::Nv24 => (self.stride as usize)
                .checked_mul(self.resolution.height as usize)?
                .checked_mul(3),
            _ => None,
        }
    }

    fn dequeue_buffer(&mut self) -> std::result::Result<V4l2Buffer, CaptureReadError> {
        self.wait_ready()?;

        // Several vendor BSPs update G_FMT/DV timings without making the
        // subscribed video fd poll as POLLPRI.  Check once per second so a
        // genuine source mode change cannot leave us dequeuing buffers with
        // stale geometry forever.  Transient ioctl failures are ignored here;
        // the capture timeout/error path remains responsible for recovery.
        if self
            .native_hdmirx_next_state_check
            .is_some_and(|next| Instant::now() >= next)
        {
            self.native_hdmirx_next_state_check = Some(Instant::now() + Duration::from_secs(1));
            if self.native_hdmirx_state_changed() {
                info!(
                    "Native HDMI RX active format/timings changed without a usable event; requesting stream re-open"
                );
                return Err(CaptureReadError::SourceChanged);
            }
        }

        let dqbuf: V4l2Buffer =
            ioctl::dqbuf(&self.fd, self.queue, MemoryType::Mmap).map_err(|error| {
                let message = error.to_string();
                let error = if error.into_errno() == Errno::EAGAIN as i32 {
                    io::Error::from(io::ErrorKind::WouldBlock)
                } else {
                    io::Error::other(format!("dqbuf failed: {}", message))
                };
                CaptureReadError::Io(error)
            })?;
        Ok(dqbuf)
    }

    /// Native HDMI NV12/BGR24 and single-planar USB UVC YUYV/NV12/RGB24/MJPEG.
    /// Actual EXPBUF/import support is probed separately; failure retains copy.
    #[cfg(any(target_arch = "aarch64", target_arch = "arm"))]
    pub(crate) fn supports_rkmpp_dmabuf(&self) -> bool {
        self.dma_layout_bytes.is_some_and(|minimum| {
            (2..=16).contains(&self.mappings.len())
                && self
                    .mappings
                    .iter()
                    .all(|planes| planes.len() == 1 && planes[0].len() >= minimum)
        })
    }

    #[cfg(any(target_arch = "aarch64", target_arch = "arm"))]
    pub(crate) fn export_dmabufs(&self) -> io::Result<Vec<(OwnedFd, usize)>> {
        if !self.supports_rkmpp_dmabuf() {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "Unsupported RKMPP DMA capture layout",
            ));
        }
        self.mappings
            .iter()
            .enumerate()
            .map(|(index, planes)| {
                let fd = ioctl::expbuf(&self.fd, self.queue, index, 0, ioctl::ExpbufFlags::CLOEXEC)
                    .map_err(|error| io::Error::other(error.to_string()))?;
                Ok((fd, planes[0].len()))
            })
            .collect()
    }

    /// Run a synchronous consumer while a buffer is dequeued. QBUF occurs only
    /// after the callback returns, including its error path. Consumers must end
    /// hardware access before returning; see hwcodec::rkmpp_dmabuf::encode.
    #[cfg(any(target_arch = "aarch64", target_arch = "arm"))]
    pub(crate) fn with_next_dmabuf<T>(
        &mut self,
        consume: impl FnOnce(usize, usize, Option<OwnedFd>) -> T,
    ) -> std::result::Result<(CaptureMeta, T), CaptureReadError> {
        let buffer = self.dequeue_buffer()?;
        let index = buffer.as_v4l2_buffer().index as usize;
        let sequence = buffer.as_v4l2_buffer().sequence as u64;
        if index >= self.mappings.len() {
            return Err(
                io::Error::new(io::ErrorKind::InvalidData, "Invalid capture buffer index").into(),
            );
        }
        let expected = self.expected_capture_bytes();
        let mapped_size = self.mappings[index][0].len();
        let native_hdmi = self.native_hdmirx_state.is_some();
        let compressed = self.format == PixelFormat::Mjpeg;
        let lease = BufferReturn(Some(|| {
            self.queue_buffer(index as u32)
                .map_err(|e| io::Error::other(e.to_string()))
        }));
        if buffer.as_v4l2_buffer().flags & v4l2r::bindings::V4L2_BUF_FLAG_ERROR != 0 {
            // A corrupt UVC frame is not a source change or a DMA failure.
            // Return it without ever letting the encoder read its payload.
            lease.finish()?;
            return Err(io::Error::from(io::ErrorKind::WouldBlock).into());
        }
        if !native_hdmi
            && buffer.as_v4l2_buffer().field != v4l2r::bindings::v4l2_field_V4L2_FIELD_NONE
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Interlaced USB DMA frames are not supported",
            )
            .into());
        }
        let mut planes = buffer.planes_iter();
        let plane = planes
            .next()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Missing DMA plane"))?;
        if planes.next().is_some() || plane.data_offset.copied().unwrap_or(0) != 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Unsupported DMA plane offset/layout",
            )
            .into());
        }
        let bytes_used = *plane.bytesused as usize;
        if !dmabuf_layout::valid_payload(compressed, bytes_used, mapped_size, expected) {
            if !native_hdmi {
                // An unexpected UVC payload is not evidence of a source mode
                // change. Disable DMA instead of reopening it indefinitely.
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Unexpected USB DMA payload length",
                )
                .into());
            }
            return Err(CaptureReadError::SourceChanged);
        }
        // UVC commonly fills vmalloc memory on the CPU. Older BSP exporters
        // cache DMA attachments without usable per-frame CPU-access sync hooks.
        // A fresh export object forces a fresh device mapping of this completed
        // frame. Reuse the actual capture allocation, not a stale attachment.
        let fresh_fd = if !native_hdmi {
            Some(
                ioctl::expbuf(
                    &self.fd,
                    self.queue,
                    index,
                    0,
                    ioctl::ExpbufFlags::CLOEXEC | ioctl::ExpbufFlags::RDWR,
                )
                .map_err(|error| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("USB DMA re-export failed: {error}"),
                    )
                })?,
            )
        } else {
            None
        };
        let output = consume(index, bytes_used, fresh_fd);
        lease.finish()?;
        Ok((
            CaptureMeta {
                bytes_used,
                sequence,
            },
            output,
        ))
    }

    pub fn next_into(
        &mut self,
        dst: &mut Vec<u8>,
    ) -> std::result::Result<CaptureMeta, CaptureReadError> {
        let dqbuf = self.dequeue_buffer()?;
        let index = dqbuf.as_v4l2_buffer().index as usize;
        let sequence = dqbuf.as_v4l2_buffer().sequence as u64;

        let mut total = 0usize;
        for (plane_idx, plane) in dqbuf.planes_iter().enumerate() {
            let bytes_used = *plane.bytesused as usize;
            let data_offset = plane.data_offset.copied().unwrap_or(0) as usize;
            if bytes_used == 0 {
                continue;
            }
            let mapping = &self.mappings[index][plane_idx];
            let start = data_offset.min(mapping.len());
            let end = (data_offset + bytes_used).min(mapping.len());
            total += end.saturating_sub(start);
        }

        dst.resize(total, 0);
        let mut cursor = 0usize;
        for (plane_idx, plane) in dqbuf.planes_iter().enumerate() {
            let bytes_used = *plane.bytesused as usize;
            let data_offset = plane.data_offset.copied().unwrap_or(0) as usize;
            if bytes_used == 0 {
                continue;
            }
            let mapping = &self.mappings[index][plane_idx];
            let start = data_offset.min(mapping.len());
            let end = (data_offset + bytes_used).min(mapping.len());
            let len = end.saturating_sub(start);
            if len == 0 {
                continue;
            }
            dst[cursor..cursor + len].copy_from_slice(&mapping[start..end]);
            cursor += len;
        }

        self.queue_buffer(index as u32)
            .map_err(|e| io::Error::other(e.to_string()))?;

        if let Some(expected) = self.expected_capture_bytes() {
            if total > 0 && total != expected {
                warn!(
                    "DQBUF bytes_used ({}) != expected ({}) for {:?} {}x{} stride={} — requesting stream re-open",
                    total,
                    expected,
                    self.format,
                    self.resolution.width,
                    self.resolution.height,
                    self.stride
                );
                return Err(CaptureReadError::SourceChanged);
            }
        }

        Ok(CaptureMeta {
            bytes_used: total,
            sequence,
        })
    }

    fn wait_ready(&self) -> std::result::Result<(), CaptureReadError> {
        if self.timeout.is_zero() {
            return Ok(());
        }
        let deadline = Instant::now() + self.timeout;
        loop {
            // Multiplex video fd (POLLIN for DQBUF, POLLPRI as fallback for
            // drivers that deliver events here) and the optional subdev fd
            // (POLLPRI only — SOURCE_CHANGE on RK628 / rkcif).
            let mut poll_fds: Vec<PollFd> = Vec::with_capacity(2);
            poll_fds.push(PollFd::new(
                self.fd.as_fd(),
                PollFlags::POLLIN | PollFlags::POLLPRI | PollFlags::POLLERR | PollFlags::POLLHUP,
            ));
            if let Some(subdev_fd) = self.subdev_fd.as_ref() {
                poll_fds.push(PollFd::new(subdev_fd.as_fd(), PollFlags::POLLPRI));
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(io::Error::new(io::ErrorKind::TimedOut, "capture timeout").into());
            }
            // `nix::poll` accepts a u16 millisecond timeout. Round sub-ms
            // durations up, and preserve the original deadline if a very long
            // timeout needs more than one poll call.
            let timeout_ms = remaining.as_millis().clamp(1, u16::MAX as u128) as u16;
            let ready = poll(&mut poll_fds, PollTimeout::from(timeout_ms))
                .map_err(|error| CaptureReadError::Io(error.into()))?;
            if ready == 0 {
                if Instant::now() >= deadline {
                    return Err(io::Error::new(io::ErrorKind::TimedOut, "capture timeout").into());
                }
                continue;
            }

            // Subdev POLLPRI fires first on rkcif/RK628 when the source-side
            // HDMI timings changed.  Native HDMI RX uses the video node and
            // is validated separately below.
            if let Some(subdev_fd) = self.subdev_fd.as_ref() {
                if let Some(revents) = poll_fds.get(1).and_then(|f| f.revents()) {
                    if revents.contains(PollFlags::POLLPRI) {
                        let drained = csi_bridge::drain_v4l2_events(subdev_fd);
                        info!(
                            "Subdev SOURCE_CHANGE detected (drained {} event(s)), \
                             requesting stream re-open",
                            drained
                        );
                        return Err(CaptureReadError::SourceChanged);
                    }
                }
            }

            if let Some(revents) = poll_fds[0].revents() {
                if revents.contains(PollFlags::POLLERR) || revents.contains(PollFlags::POLLHUP) {
                    debug!(
                        "capture poll: video revents={:?} (ERR/HUP) — requesting stream re-open",
                        revents
                    );
                    return Err(CaptureReadError::SourceChanged);
                }
                if revents.contains(PollFlags::POLLPRI) {
                    let drained = csi_bridge::drain_v4l2_events(&self.fd);
                    if self.native_hdmirx_state_unchanged() {
                        debug!(
                            "Ignoring {} spurious native HDMI RX SOURCE_CHANGE event(s): active format/timings are unchanged",
                            drained
                        );
                        if revents.contains(PollFlags::POLLIN) {
                            return Ok(());
                        }
                        continue;
                    }
                    info!(
                        "Video-node SOURCE_CHANGE detected (drained {} event(s)), \
                         requesting stream re-open",
                        drained
                    );
                    return Err(CaptureReadError::SourceChanged);
                }
                if !revents.contains(PollFlags::POLLIN) {
                    // rkcif + RK628: the driver may wake `poll` after internally
                    // invalidating queued buffers without queueing a V4L2 event.
                    // Treat like SOURCE_CHANGE so we STREAMOFF / re-S_FMT.
                    debug!(
                        "capture poll: ready={} video revents={:?} (no POLLIN) — requesting stream re-open",
                        ready, revents
                    );
                    return Err(CaptureReadError::SourceChanged);
                }
                return Ok(());
            }

            debug!(
                "capture poll: ready={} but video revents unavailable — requesting stream re-open",
                ready
            );
            return Err(CaptureReadError::SourceChanged);
        }
    }

    fn native_hdmirx_state_unchanged(&self) -> bool {
        let Some(expected) = self.native_hdmirx_state.as_ref() else {
            return false;
        };
        let Ok(current_fmt) = ioctl::g_fmt::<V4l2rFormat>(&self.fd, self.queue) else {
            return false;
        };
        if !expected.format_matches(
            current_fmt.width,
            current_fmt.height,
            current_fmt.pixelformat,
        ) {
            return false;
        }

        let observed_timings = ioctl::query_dv_timings::<v4l2_dv_timings>(&self.fd)
            .ok()
            .and_then(|timings| dv_timings_signature(&timings));
        expected.timings_match(observed_timings).unwrap_or(false)
    }

    fn native_hdmirx_state_changed(&self) -> bool {
        let Some(expected) = self.native_hdmirx_state.as_ref() else {
            return false;
        };
        let Ok(current_fmt) = ioctl::g_fmt::<V4l2rFormat>(&self.fd, self.queue) else {
            return false;
        };
        if !expected.format_matches(
            current_fmt.width,
            current_fmt.height,
            current_fmt.pixelformat,
        ) {
            return true;
        }

        let observed_timings = ioctl::query_dv_timings::<v4l2_dv_timings>(&self.fd)
            .ok()
            .and_then(|timings| dv_timings_signature(&timings));
        expected
            .timings_match(observed_timings)
            .is_some_and(|matches| !matches)
    }

    fn queue_all_buffers(&mut self) -> Result<()> {
        for index in 0..self.mappings.len() as u32 {
            self.queue_buffer(index)?;
        }
        Ok(())
    }

    fn queue_buffer(&self, index: u32) -> Result<()> {
        let handle = MmapHandle;
        let planes = self.mappings[index as usize]
            .iter()
            .map(|mapping| {
                let mut plane = QBufPlane::new_from_handle(&handle, 0);
                plane.0.length = mapping.len() as u32;
                plane
            })
            .collect();
        let mut qbuf: QBuffer<MmapHandle> = QBuffer::new(self.queue, index);
        qbuf.planes = planes;
        ioctl::qbuf::<_, ()>(&self.fd, qbuf)
            .map_err(|e| AppError::VideoError(format!("Failed to queue buffer: {}", e)))?;
        Ok(())
    }
}

#[cfg(any(test, target_arch = "aarch64", target_arch = "arm"))]
struct BufferReturn<F: FnOnce() -> io::Result<()>>(Option<F>);

#[cfg(any(test, target_arch = "aarch64", target_arch = "arm"))]
impl<F: FnOnce() -> io::Result<()>> BufferReturn<F> {
    fn finish(mut self) -> io::Result<()> {
        self.0.take().expect("capture lease already returned")()
    }
}

#[cfg(any(test, target_arch = "aarch64", target_arch = "arm"))]
impl<F: FnOnce() -> io::Result<()>> Drop for BufferReturn<F> {
    fn drop(&mut self) {
        if let Some(return_buffer) = self.0.take() {
            if let Err(error) = return_buffer() {
                warn!("Failed to return leased capture buffer: {}", error);
            }
        }
    }
}

#[cfg(test)]
mod dma_lease_tests {
    use super::*;
    use std::cell::RefCell;

    #[test]
    fn returns_buffer_once_after_consumer_and_does_not_retry_failed_qbuf() {
        let operations = RefCell::new(Vec::new());
        let lease = BufferReturn(Some(|| {
            operations.borrow_mut().push("qbuf");
            Err(io::Error::other("device lost"))
        }));
        operations.borrow_mut().push("encode completed");
        assert!(lease.finish().is_err());
        assert_eq!(*operations.borrow(), ["encode completed", "qbuf"]);
    }

    #[test]
    fn returns_buffer_on_validation_error_or_unwind() {
        let returns = std::cell::Cell::new(0);
        {
            let _lease = BufferReturn(Some(|| {
                returns.set(returns.get() + 1);
                Ok(())
            }));
        }
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _lease = BufferReturn(Some(|| {
                returns.set(returns.get() + 1);
                Ok(())
            }));
            panic!("consumer panic");
        }));
        assert_eq!(returns.get(), 2);
    }
}

impl Drop for CaptureStream {
    fn drop(&mut self) {
        // Release ordering matters on rkcif: a subsequent open()/S_FMT from a
        // freshly-constructed stream returns EBUSY if the previous capture has
        // not fully relinquished its buffers.  Mirror the ustreamer teardown
        // order:
        //   1. STREAMOFF            (stop DMA)
        //   2. unsubscribe_all      (no further DQEVENT paths)
        //   3. munmap via Drop      (release buffer mappings)
        //   4. REQBUFS count=0      (free kernel buffer list)
        //   5. close(fd)            (implicit on File Drop)
        if let Err(e) = ioctl::streamoff(&self.fd, self.queue) {
            debug!("Failed to stop capture stream: {}", e);
        }
        if let Err(e) = ioctl::unsubscribe_all_events(&self.fd) {
            debug!("Failed to unsubscribe V4L2 events: {}", e);
        }
        // Explicit munmap *before* REQBUFS(0) — the kernel refuses to free the
        // buffer list while mappings are outstanding.
        self.mappings.clear();
        if let Err(e) = ioctl::reqbufs::<v4l2_requestbuffers>(
            &self.fd,
            self.queue,
            MemoryType::Mmap,
            0,
            MemoryConsistency::empty(),
        ) {
            debug!("Failed to release capture buffers: {}", e);
        }
    }
}

/// Result of a successful `VIDIOC_QUERY_DV_TIMINGS` + `VIDIOC_S_DV_TIMINGS`
/// probe.  Used by the CSI bridge path to override the requested resolution
/// with the source-reported geometry before `S_FMT`.
#[derive(Debug, Clone, Copy)]
struct DvTimingsMode {
    width: u32,
    height: u32,
    #[allow(dead_code)]
    fps: Option<f64>,
    signature: Option<DvTimingsSignature>,
}

#[derive(Debug, Clone, Copy)]
struct NativeHdmirxState {
    width: u32,
    height: u32,
    pixelformat: V4l2rPixelFormat,
    timings: Option<DvTimingsSignature>,
}

impl NativeHdmirxState {
    fn format_matches(self, width: u32, height: u32, pixelformat: V4l2rPixelFormat) -> bool {
        self.width == width && self.height == height && self.pixelformat == pixelformat
    }

    /// `None` means the expected state contains timings but the current
    /// timings could not be observed. Event handling treats that uncertainty
    /// conservatively as changed; periodic fallback probing ignores it so a
    /// single transient ioctl failure cannot tear down a healthy stream.
    fn timings_match(self, observed: Option<DvTimingsSignature>) -> Option<bool> {
        match (self.timings, observed) {
            (None, _) => Some(true),
            (Some(expected), Some(current)) => Some(current.matches(expected)),
            (Some(_), None) => None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct DvTimingsSignature {
    width: u32,
    height: u32,
    interlaced: bool,
}

impl DvTimingsSignature {
    fn matches(self, other: Self) -> bool {
        self.width == other.width
            && self.height == other.height
            && self.interlaced == other.interlaced
    }
}

fn dv_timings_signature(timings: &v4l2_dv_timings) -> Option<DvTimingsSignature> {
    let timings_type = timings.type_;
    if timings_type != V4L2_DV_BT_656_1120 {
        return None;
    }
    let bt = unsafe { timings.__bindgen_anon_1.bt };
    let width = bt.width;
    let height = bt.height;
    Some(DvTimingsSignature {
        width,
        height,
        interlaced: bt.interlaced != 0,
    })
}

/// Probe DV timings from the source and latch them into the driver.
///
/// Mirrors PiKVM/ustreamer's `src_hdmi_open_sequence`:
///   1. `VIDIOC_QUERY_DV_TIMINGS` — active-probe the source.
///   2. On success, `VIDIOC_S_DV_TIMINGS` — commit so that subsequent
///      `S_FMT` is accepted at the matching geometry.
///   3. Return the timings for the caller to feed into `S_FMT`.
///
/// Errno mapping (see `V4L2_CID_DV_RX_POWER_PRESENT` semantics):
///   * `ENOLINK`    → `NoCable`  (TMDS clock absent, cable unplugged)
///   * `ENOLCK`     → `NoSync`   (TMDS present, timings unstable)
///   * `ERANGE`     → `OutOfRange` (timings outside hardware caps)
///   * `ENODATA`    → `NoSignal` (driver says "no DV timings support on
///                                this input", e.g. EDID not applied yet)
///   * anything else → `NoSignal` (fallback, keeps the retry loop going)
fn probe_dv_timings(fd: &File, apply: bool) -> Result<DvTimingsMode> {
    let timings: v4l2_dv_timings = match ioctl::query_dv_timings(fd) {
        Ok(t) => t,
        Err(err) => {
            let status = match &err {
                QueryDvTimingsError::NoLink => SignalStatus::NoCable,
                QueryDvTimingsError::UnstableSignal => SignalStatus::NoSync,
                QueryDvTimingsError::IoctlError(Errno::ERANGE) => SignalStatus::OutOfRange,
                QueryDvTimingsError::Unsupported => SignalStatus::NoSignal,
                // I2C-layer failures between rkcif and the RK628 bridge
                // (`ret=-110`/-121/-5) typically mean the bridge is in the
                // middle of a PHY re-lock, not that the source is gone.
                // Classify them as `NoSync` so the upper layer keeps retrying
                // on the short end of the back-off ladder.
                QueryDvTimingsError::IoctlError(Errno::EIO)
                | QueryDvTimingsError::IoctlError(Errno::EREMOTEIO)
                | QueryDvTimingsError::IoctlError(Errno::ETIMEDOUT) => SignalStatus::NoSync,
                QueryDvTimingsError::IoctlError(_) => SignalStatus::NoSignal,
            };
            debug!(
                "VIDIOC_QUERY_DV_TIMINGS failed: {} -> SignalStatus::{:?}",
                err, status
            );
            return Err(AppError::CaptureNoSignal {
                kind: status.as_str().to_string(),
            });
        }
    };

    // `v4l2_dv_timings` is a packed union; copy the scalar fields out to
    // aligned locals before formatting / comparing to avoid UB (and the
    // rustc E0793 "reference to field of packed struct is unaligned" error).
    let timings_type: u32 = timings.type_;
    if timings_type != V4L2_DV_BT_656_1120 {
        warn!(
            "QUERY_DV_TIMINGS returned unknown type {}, treating as NoSignal",
            timings_type
        );
        return Err(AppError::CaptureNoSignal {
            kind: SignalStatus::NoSignal.as_str().to_string(),
        });
    }

    let bt = unsafe { timings.__bindgen_anon_1.bt };
    let bt_width: u32 = bt.width;
    let bt_height: u32 = bt.height;
    let bt_pixelclock: u64 = bt.pixelclock;
    let bt_hfrontporch: u32 = bt.hfrontporch;
    let bt_hsync: u32 = bt.hsync;
    let bt_hbackporch: u32 = bt.hbackporch;
    let bt_vfrontporch: u32 = bt.vfrontporch;
    let bt_vsync: u32 = bt.vsync;
    let bt_vbackporch: u32 = bt.vbackporch;

    if bt_width == 0 || bt_height == 0 || bt_width <= 64 || bt_height <= 64 {
        warn!(
            "QUERY_DV_TIMINGS returned degenerate {}x{}, treating as NoSignal",
            bt_width, bt_height
        );
        return Err(AppError::CaptureNoSignal {
            kind: SignalStatus::NoSignal.as_str().to_string(),
        });
    }

    // Latch the detected timings so subsequent S_FMT / STREAMON use the
    // right pixel clock + blanking.  Failure here is *not* fatal on some
    // drivers (rkcif doesn't implement S_DV_TIMINGS per-output-device, only
    // on the bridging subdev), so degrade to a warning and keep going.
    if apply {
        if let Err(e) = ioctl::s_dv_timings::<_, v4l2_dv_timings>(fd, timings) {
            debug!(
                "VIDIOC_S_DV_TIMINGS failed ({}), continuing with queried timings for S_FMT",
                e
            );
        }
    }

    let fps = dv_timings_fps_from_scalars(
        bt_width,
        bt_height,
        bt_hfrontporch + bt_hsync + bt_hbackporch,
        bt_vfrontporch + bt_vsync + bt_vbackporch,
        bt_pixelclock,
    );
    info!(
        "DV timings locked: {}x{} @ {} (pix_clk={})",
        bt_width,
        bt_height,
        fps.map(|f| format!("{:.2} fps", f))
            .unwrap_or_else(|| "?fps".to_string()),
        bt_pixelclock
    );

    Ok(DvTimingsMode {
        width: bt_width,
        height: bt_height,
        fps,
        signature: dv_timings_signature(&timings),
    })
}

fn dv_timings_fps_from_scalars(
    width: u32,
    height: u32,
    h_blanking: u32,
    v_blanking: u32,
    pixelclock: u64,
) -> Option<f64> {
    let total_h = (width + h_blanking) as u64;
    let total_v = (height + v_blanking) as u64;
    let denom = total_h.checked_mul(total_v)?;
    if denom == 0 || pixelclock == 0 {
        return None;
    }
    Some(pixelclock as f64 / denom as f64)
}

fn set_fps(fd: &File, queue: QueueType, fps: u32) -> std::result::Result<(), ioctl::GParmError> {
    let mut params = unsafe { std::mem::zeroed::<v4l2_streamparm>() };
    params.type_ = queue as u32;
    params.parm = v4l2_streamparm__bindgen_ty_1 {
        capture: v4l2r::bindings::v4l2_captureparm {
            timeperframe: v4l2r::bindings::v4l2_fract {
                numerator: 1,
                denominator: fps,
            },
            ..unsafe { std::mem::zeroed() }
        },
    };

    let _actual: v4l2_streamparm = ioctl::s_parm(fd, params)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{open_capture_device, DvTimingsSignature, NativeHdmirxState};
    use crate::video::format::PixelFormat;

    fn timing() -> DvTimingsSignature {
        DvTimingsSignature {
            width: 1920,
            height: 1080,
            interlaced: false,
        }
    }

    #[test]
    fn timing_match_uses_only_active_geometry_and_scan_mode() {
        assert!(timing().matches(timing()));

        let mut different_width = timing();
        different_width.width = 1280;
        assert!(!timing().matches(different_width));

        let mut interlaced = timing();
        interlaced.interlaced = true;
        assert!(!timing().matches(interlaced));
    }

    #[test]
    fn native_hdmirx_state_distinguishes_spurious_and_real_changes() {
        let bgr24 = PixelFormat::Bgr24.to_v4l2r();
        let state = NativeHdmirxState {
            width: 1920,
            height: 1080,
            pixelformat: bgr24,
            timings: Some(timing()),
        };

        assert!(state.format_matches(1920, 1080, bgr24));
        assert!(!state.format_matches(1280, 720, bgr24));
        assert!(!state.format_matches(1920, 1080, PixelFormat::Nv12.to_v4l2r()));
        assert_eq!(state.timings_match(Some(timing())), Some(true));
        let mut interlaced = timing();
        interlaced.interlaced = true;
        assert_eq!(state.timings_match(Some(interlaced)), Some(false));
        assert_eq!(state.timings_match(None), None);

        let no_timing_state = NativeHdmirxState {
            timings: None,
            ..state
        };
        assert_eq!(no_timing_state.timings_match(None), Some(true));
    }

    #[test]
    fn capture_device_handles_are_non_blocking() {
        let temp = tempfile::NamedTempFile::new().expect("create temporary device file");
        let opened = open_capture_device(temp.path()).expect("open capture device");
        let flags =
            nix::fcntl::fcntl(&opened, nix::fcntl::FcntlArg::F_GETFL).expect("read file flags");

        assert_ne!(flags & libc::O_NONBLOCK, 0);
    }
}
