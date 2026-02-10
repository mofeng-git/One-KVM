//! V4L2 capture implementation using v4l2r (ioctl layer).

use std::fs::File;
use std::io;
use std::os::fd::AsFd;
use std::path::Path;
use std::time::Duration;

use nix::poll::{poll, PollFd, PollFlags, PollTimeout};
use tracing::{debug, warn};
use v4l2r::bindings::{v4l2_requestbuffers, v4l2_streamparm, v4l2_streamparm__bindgen_ty_1};
use v4l2r::ioctl::{
    self, Capabilities, Capability as V4l2rCapability, MemoryConsistency, PlaneMapping,
    QBufPlane, QBuffer, QueryBuffer, V4l2Buffer,
};
use v4l2r::memory::{MemoryType, MmapHandle};
use v4l2r::{Format as V4l2rFormat, PixelFormat as V4l2rPixelFormat, QueueType};

use crate::error::{AppError, Result};
use crate::video::format::{PixelFormat, Resolution};

/// Metadata for a captured frame.
#[derive(Debug, Clone, Copy)]
pub struct CaptureMeta {
    pub bytes_used: usize,
    pub sequence: u64,
}

/// V4L2 capture stream backed by v4l2r ioctl.
pub struct V4l2rCaptureStream {
    fd: File,
    queue: QueueType,
    resolution: Resolution,
    format: PixelFormat,
    stride: u32,
    timeout: Duration,
    mappings: Vec<Vec<PlaneMapping>>,
}

impl V4l2rCaptureStream {
    pub fn open(
        device_path: impl AsRef<Path>,
        resolution: Resolution,
        format: PixelFormat,
        fps: u32,
        buffer_count: u32,
        timeout: Duration,
    ) -> Result<Self> {
        let mut fd = File::options()
            .read(true)
            .write(true)
            .open(device_path.as_ref())
            .map_err(|e| AppError::VideoError(format!("Failed to open device: {}", e)))?;

        let caps: V4l2rCapability = ioctl::querycap(&fd)
            .map_err(|e| AppError::VideoError(format!("Failed to query capabilities: {}", e)))?;
        let caps_flags = caps.device_caps();

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

        let mut fmt: V4l2rFormat = ioctl::g_fmt(&fd, queue).map_err(|e| {
            AppError::VideoError(format!("Failed to get device format: {}", e))
        })?;

        fmt.width = resolution.width;
        fmt.height = resolution.height;
        fmt.pixelformat = V4l2rPixelFormat::from(&format.to_fourcc());

        let actual_fmt: V4l2rFormat = ioctl::s_fmt(&mut fd, (queue, &fmt)).map_err(|e| {
            AppError::VideoError(format!("Failed to set device format: {}", e))
        })?;

        let actual_resolution = Resolution::new(actual_fmt.width, actual_fmt.height);
        let actual_format = PixelFormat::from_v4l2r(actual_fmt.pixelformat).unwrap_or(format);

        let stride = actual_fmt
            .plane_fmt
            .get(0)
            .map(|p| p.bytesperline)
            .unwrap_or_else(|| match actual_format.bytes_per_pixel() {
                Some(bpp) => actual_resolution.width * bpp as u32,
                None => actual_resolution.width,
            });

        if fps > 0 {
            if let Err(e) = set_fps(&fd, queue, fps) {
                warn!("Failed to set hardware FPS: {}", e);
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
                    AppError::VideoError(format!(
                        "Failed to mmap buffer {}: {}",
                        index, e
                    ))
                })?;
                plane_maps.push(mapping);
            }
            mappings.push(plane_maps);
        }

        let mut stream = Self {
            fd,
            queue,
            resolution: actual_resolution,
            format: actual_format,
            stride,
            timeout,
            mappings,
        };

        stream.queue_all_buffers()?;
        ioctl::streamon(&stream.fd, stream.queue).map_err(|e| {
            AppError::VideoError(format!("Failed to start capture stream: {}", e))
        })?;

        Ok(stream)
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
        self.wait_ready()?;

        let dqbuf: V4l2Buffer = ioctl::dqbuf(&self.fd, self.queue).map_err(|e| {
            io::Error::new(io::ErrorKind::Other, format!("dqbuf failed: {}", e))
        })?;
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
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;

        Ok(CaptureMeta {
            bytes_used: total,
            sequence,
        })
    }

    fn wait_ready(&self) -> io::Result<()> {
        if self.timeout.is_zero() {
            return Ok(());
        }
        let mut fds = [PollFd::new(self.fd.as_fd(), PollFlags::POLLIN)];
        let timeout_ms = self.timeout.as_millis().min(u16::MAX as u128) as u16;
        let ready = poll(&mut fds, PollTimeout::from(timeout_ms))?;
        if ready == 0 {
            return Err(io::Error::new(io::ErrorKind::TimedOut, "capture timeout"));
        }
        Ok(())
    }

    fn queue_all_buffers(&mut self) -> Result<()> {
        for index in 0..self.mappings.len() as u32 {
            self.queue_buffer(index)?;
        }
        Ok(())
    }

    fn queue_buffer(&mut self, index: u32) -> Result<()> {
        let handle = MmapHandle::default();
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

impl Drop for V4l2rCaptureStream {
    fn drop(&mut self) {
        if let Err(e) = ioctl::streamoff(&self.fd, self.queue) {
            debug!("Failed to stop capture stream: {}", e);
        }
    }
}

fn set_fps(fd: &File, queue: QueueType, fps: u32) -> Result<()> {
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

    let _actual: v4l2_streamparm = ioctl::s_parm(fd, params)
        .map_err(|e| AppError::VideoError(format!("Failed to set FPS: {}", e)))?;
    Ok(())
}
