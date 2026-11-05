//! Safe wrapper for the VIDIOC_(D)QBUF and VIDIOC_QUERYBUF ioctls.
use nix::errno::Errno;
use nix::libc::{suseconds_t, time_t};
use nix::sys::time::{TimeVal, TimeValLike};
use std::convert::TryFrom;
use std::fmt::Debug;
use std::os::unix::io::AsRawFd;
use thiserror::Error;

use crate::bindings;
use crate::ioctl::ioctl_and_convert;
use crate::ioctl::BufferFlags;
use crate::ioctl::IoctlConvertError;
use crate::ioctl::IoctlConvertResult;
use crate::ioctl::UncheckedV4l2Buffer;
use crate::memory::Memory;
use crate::memory::PlaneHandle;
use crate::QueueType;

#[derive(Debug, Error)]
pub enum QBufIoctlError {
    #[error("invalid number of planes specified for the buffer: got {0}, expected {1}")]
    NumPlanesMismatch(usize, usize),
    #[error("data offset specified while using the single-planar API")]
    DataOffsetNotSupported,
    #[error("unexpected ioctl error: {0}")]
    Other(Errno),
}

impl From<Errno> for QBufIoctlError {
    fn from(errno: Errno) -> Self {
        Self::Other(errno)
    }
}

impl From<QBufIoctlError> for Errno {
    fn from(err: QBufIoctlError) -> Self {
        match err {
            QBufIoctlError::NumPlanesMismatch(_, _) => Errno::EINVAL,
            QBufIoctlError::DataOffsetNotSupported => Errno::EINVAL,
            QBufIoctlError::Other(e) => e,
        }
    }
}

/// Representation of a single plane of a V4L2 buffer.
pub struct QBufPlane(pub bindings::v4l2_plane);

impl QBufPlane {
    // TODO remove as this is not safe - we should always specify a handle.
    pub fn new(bytes_used: usize) -> Self {
        QBufPlane(bindings::v4l2_plane {
            bytesused: bytes_used as u32,
            data_offset: 0,
            ..Default::default()
        })
    }

    pub fn new_from_handle<H: PlaneHandle>(handle: &H, bytes_used: usize) -> Self {
        let mut plane = Self::new(bytes_used);
        handle.fill_v4l2_plane(&mut plane.0);
        plane
    }
}

impl Debug for QBufPlane {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("QBufPlane")
            .field("bytesused", &self.0.bytesused)
            .field("data_offset", &self.0.data_offset)
            .finish()
    }
}

/// Contains all the information that can be passed to the `qbuf` ioctl.
// TODO Change this to contain a v4l2_buffer, and create constructors/methods
// to change it? Then during qbuf we just need to set m.planes to planes
// (after resizing it to 8) and we are good to use it as-is.
// We could even turn the trait into AsRef<v4l2_buffer> for good measure.
#[derive(Debug)]
pub struct QBuffer<H: PlaneHandle> {
    index: u32,
    queue: QueueType,
    pub flags: BufferFlags,
    pub field: u32,
    pub sequence: u32,
    pub timestamp: TimeVal,
    pub planes: Vec<QBufPlane>,
    pub _h: std::marker::PhantomData<H>,
}

impl<H: PlaneHandle> QBuffer<H> {
    pub fn new(queue: QueueType, index: u32) -> Self {
        QBuffer {
            index,
            queue,
            flags: Default::default(),
            field: Default::default(),
            sequence: Default::default(),
            timestamp: TimeVal::zero(),
            planes: Vec::new(),
            _h: std::marker::PhantomData,
        }
    }
}

impl<H: PlaneHandle> QBuffer<H> {
    pub fn set_timestamp(mut self, sec: time_t, usec: suseconds_t) -> Self {
        self.timestamp = TimeVal::new(sec, usec);
        self
    }
}

impl<H: PlaneHandle> From<QBuffer<H>> for UncheckedV4l2Buffer {
    fn from(qbuf: QBuffer<H>) -> Self {
        let mut v4l2_buf = UncheckedV4l2Buffer::new_for_querybuf(qbuf.queue, Some(qbuf.index));
        v4l2_buf.0.index = qbuf.index;
        v4l2_buf.0.type_ = qbuf.queue as u32;
        v4l2_buf.0.memory = H::Memory::MEMORY_TYPE as u32;
        v4l2_buf.0.flags = qbuf.flags.bits();
        v4l2_buf.0.field = qbuf.field;
        v4l2_buf.0.sequence = qbuf.sequence;
        v4l2_buf.0.timestamp.tv_sec = qbuf.timestamp.tv_sec();
        v4l2_buf.0.timestamp.tv_usec = qbuf.timestamp.tv_usec();
        if let Some(planes) = &mut v4l2_buf.1 {
            for (dst_plane, src_plane) in planes.iter_mut().zip(qbuf.planes.into_iter()) {
                *dst_plane = src_plane.0;
            }
        } else {
            let plane = &qbuf.planes[0];

            v4l2_buf.0.length = plane.0.length;
            v4l2_buf.0.bytesused = plane.0.bytesused;
            v4l2_buf.0.m = (&plane.0.m, H::Memory::MEMORY_TYPE).into();
        }

        v4l2_buf
    }
}

#[doc(hidden)]
mod ioctl {
    use crate::bindings::v4l2_buffer;
    nix::ioctl_readwrite!(vidioc_querybuf, b'V', 9, v4l2_buffer);
    nix::ioctl_readwrite!(vidioc_qbuf, b'V', 15, v4l2_buffer);
    nix::ioctl_readwrite!(vidioc_dqbuf, b'V', 17, v4l2_buffer);
    nix::ioctl_readwrite!(vidioc_prepare_buf, b'V', 93, v4l2_buffer);
}

pub type QBufError<CE> = IoctlConvertError<QBufIoctlError, CE>;
pub type QBufResult<O, CE> = IoctlConvertResult<O, QBufIoctlError, CE>;

/// Safe wrapper around the `VIDIOC_QBUF` ioctl.
///
/// TODO: `qbuf` should be unsafe! The following invariants need to be guaranteed
/// by the caller:
///
/// For MMAP buffers, any mapping must not be accessed by the caller (or any
/// mapping must be unmapped before queueing?). Also if the buffer has been
/// DMABUF-exported, its consumers must likewise not access it.
///
/// For DMABUF buffers, the FD must not be duplicated and accessed anywhere else.
///
/// For USERPTR buffers, things are most tricky. Not only must the data not be
/// accessed by anyone else, the caller also needs to guarantee that the backing
/// memory won't be freed until the corresponding buffer is returned by either
/// `dqbuf` or `streamoff`.
pub fn qbuf<I, O>(fd: &impl AsRawFd, buffer: I) -> QBufResult<O, O::Error>
where
    I: Into<UncheckedV4l2Buffer>,
    O: TryFrom<UncheckedV4l2Buffer>,
    O::Error: std::fmt::Debug,
{
    let mut v4l2_buf: UncheckedV4l2Buffer = buffer.into();

    ioctl_and_convert(
        unsafe { ioctl::vidioc_qbuf(fd.as_raw_fd(), v4l2_buf.as_mut()) }
            .map(|_| v4l2_buf)
            .map_err(Into::into),
    )
}

/// Safe wrapper around the `VIDIOC_PREPARE_BUF` ioctl.
pub fn prepare_buf<I, O>(fd: &impl AsRawFd, buffer: I) -> QBufResult<O, O::Error>
where
    I: Into<UncheckedV4l2Buffer>,
    O: TryFrom<UncheckedV4l2Buffer>,
    O::Error: std::fmt::Debug,
{
    let mut v4l2_buf: UncheckedV4l2Buffer = buffer.into();

    ioctl_and_convert(
        unsafe { ioctl::vidioc_prepare_buf(fd.as_raw_fd(), v4l2_buf.as_mut()) }
            .map(|_| v4l2_buf)
            .map_err(Into::into),
    )
}
