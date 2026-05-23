//! Safe wrapper for the `VIDIOC_REQBUFS` ioctl.
use crate::bindings;
use crate::bindings::v4l2_create_buffers;
use crate::bindings::v4l2_format;
use crate::bindings::v4l2_requestbuffers;
use crate::memory::MemoryType;
use crate::QueueType;
use bitflags::bitflags;
use nix::{self, errno::Errno};
use std::os::unix::io::AsRawFd;
use thiserror::Error;

bitflags! {
    /// Flags returned by the `VIDIOC_REQBUFS` ioctl into the `capabilities`
    /// field of `struct v4l2_requestbuffers`.
    #[derive(Clone, Copy, Debug)]
    pub struct BufferCapabilities: u32 {
        const SUPPORTS_MMAP = bindings::V4L2_BUF_CAP_SUPPORTS_MMAP;
        const SUPPORTS_USERPTR = bindings::V4L2_BUF_CAP_SUPPORTS_USERPTR;
        const SUPPORTS_DMABUF = bindings::V4L2_BUF_CAP_SUPPORTS_DMABUF;
        const SUPPORTS_REQUESTS = bindings::V4L2_BUF_CAP_SUPPORTS_REQUESTS;
        const SUPPORTS_ORPHANED_BUFS = bindings::V4L2_BUF_CAP_SUPPORTS_ORPHANED_BUFS;
        const SUPPORTS_M2M_HOLD_CAPTURE_BUF = bindings::V4L2_BUF_CAP_SUPPORTS_M2M_HOLD_CAPTURE_BUF;
        const SUPPORTS_MMAP_CACHE_HINTS = bindings::V4L2_BUF_CAP_SUPPORTS_MMAP_CACHE_HINTS;
    }
}

bitflags! {
    /// Memory Consistency Flags passed to the `VIDIOC_REQBUFS` ioctl in the `flags`
    /// field of `struct v4l2_requestbuffers`.
    #[derive(Clone, Copy, Debug)]
    pub struct MemoryConsistency: u8 {
        const MEMORY_FLAG_NON_COHERENT = bindings::V4L2_MEMORY_FLAG_NON_COHERENT as u8;
    }
}

impl From<v4l2_requestbuffers> for () {
    fn from(_reqbufs: v4l2_requestbuffers) -> Self {}
}

/// In case we are just interested in the number of buffers that `reqbufs`
/// created.
impl From<v4l2_requestbuffers> for usize {
    fn from(reqbufs: v4l2_requestbuffers) -> Self {
        reqbufs.count as usize
    }
}

/// If we just want to query the buffer capabilities.
impl From<v4l2_requestbuffers> for BufferCapabilities {
    fn from(reqbufs: v4l2_requestbuffers) -> Self {
        BufferCapabilities::from_bits_truncate(reqbufs.capabilities)
    }
}

/// Full result of the `reqbufs` ioctl.
pub struct RequestBuffers {
    pub count: u32,
    pub capabilities: BufferCapabilities,
    pub flags: MemoryConsistency,
}

impl From<v4l2_requestbuffers> for RequestBuffers {
    fn from(reqbufs: v4l2_requestbuffers) -> Self {
        RequestBuffers {
            count: reqbufs.count,
            capabilities: BufferCapabilities::from_bits_truncate(reqbufs.capabilities),
            flags: MemoryConsistency::from_bits_truncate(reqbufs.flags),
        }
    }
}

#[doc(hidden)]
mod ioctl {
    use crate::bindings::v4l2_create_buffers;
    use crate::bindings::v4l2_requestbuffers;

    nix::ioctl_readwrite!(vidioc_reqbufs, b'V', 8, v4l2_requestbuffers);
    nix::ioctl_readwrite!(vidioc_create_bufs, b'V', 92, v4l2_create_buffers);
}

#[derive(Debug, Error)]
pub enum ReqbufsError {
    #[error("invalid buffer ({0}) or memory type ({1:?}) requested")]
    InvalidBufferType(QueueType, MemoryType),
    #[error("ioctl error: {0}")]
    IoctlError(nix::Error),
}

impl From<ReqbufsError> for Errno {
    fn from(err: ReqbufsError) -> Self {
        match err {
            ReqbufsError::InvalidBufferType(_, _) => Errno::EINVAL,
            ReqbufsError::IoctlError(e) => e,
        }
    }
}

/// Safe wrapper around the `VIDIOC_REQBUFS` ioctl.
pub fn reqbufs<O: From<v4l2_requestbuffers>>(
    fd: &impl AsRawFd,
    queue: QueueType,
    memory: MemoryType,
    count: u32,
    flags: MemoryConsistency,
) -> Result<O, ReqbufsError> {
    let mut reqbufs = v4l2_requestbuffers {
        count,
        type_: queue as u32,
        memory: memory as u32,
        flags: flags.bits(),
        ..Default::default()
    };

    match unsafe { ioctl::vidioc_reqbufs(fd.as_raw_fd(), &mut reqbufs) } {
        Ok(_) => Ok(O::from(reqbufs)),
        Err(Errno::EINVAL) => Err(ReqbufsError::InvalidBufferType(queue, memory)),
        Err(e) => Err(ReqbufsError::IoctlError(e)),
    }
}

#[derive(Debug, Error)]
pub enum CreateBufsError {
    #[error("no memory available to allocate MMAP buffers")]
    NoMem,
    #[error("invalid format or memory type requested")]
    Invalid,
    #[error("ioctl error: {0}")]
    IoctlError(nix::Error),
}

impl From<CreateBufsError> for Errno {
    fn from(err: CreateBufsError) -> Self {
        match err {
            CreateBufsError::NoMem => Errno::ENOMEM,
            CreateBufsError::Invalid => Errno::EINVAL,
            CreateBufsError::IoctlError(e) => e,
        }
    }
}

/// Safe wrapper around the `VIDIOC_CREATE_BUFS` ioctl.
pub fn create_bufs<F: Into<v4l2_format>, O: From<v4l2_create_buffers>>(
    fd: &impl AsRawFd,
    count: u32,
    memory: MemoryType,
    format: F,
) -> Result<O, CreateBufsError> {
    let mut create_bufs = v4l2_create_buffers {
        count,
        memory: memory as u32,
        format: format.into(),
        ..Default::default()
    };

    match unsafe { ioctl::vidioc_create_bufs(fd.as_raw_fd(), &mut create_bufs) } {
        Ok(_) => Ok(O::from(create_bufs)),
        Err(Errno::ENOMEM) => Err(CreateBufsError::NoMem),
        Err(Errno::EINVAL) => Err(CreateBufsError::Invalid),
        Err(e) => Err(CreateBufsError::IoctlError(e)),
    }
}
