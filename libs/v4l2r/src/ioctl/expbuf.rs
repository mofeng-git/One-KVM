//! Safe wrapper for the `VIDIOC_EXPBUF` ioctl.
use bitflags::bitflags;
use nix::errno::Errno;
use nix::fcntl::OFlag;
use std::os::unix::io::{AsRawFd, FromRawFd};
use thiserror::Error;

use crate::bindings::v4l2_exportbuffer;
use crate::QueueType;

bitflags! {
    /// Flags that can be passed when exporting the buffer.
    #[derive(Clone, Copy, Debug)]
    pub struct ExpbufFlags: u32 {
        const CLOEXEC = OFlag::O_CLOEXEC.bits() as u32;
        const RDONLY = OFlag::O_RDONLY.bits() as u32;
        const WRONLY = OFlag::O_WRONLY.bits() as u32;
        const RDWR = OFlag::O_RDWR.bits() as u32;
    }
}

#[doc(hidden)]
mod ioctl {
    use crate::bindings::v4l2_exportbuffer;
    nix::ioctl_readwrite!(vidioc_expbuf, b'V', 16, v4l2_exportbuffer);
}

#[derive(Debug, Error)]
pub enum ExpbufError {
    #[error("ioctl error: {0}")]
    IoctlError(#[from] Errno),
}

impl From<ExpbufError> for Errno {
    fn from(err: ExpbufError) -> Self {
        match err {
            ExpbufError::IoctlError(e) => e,
        }
    }
}

/// Safe wrapper around the `VIDIOC_EXPBUF` ioctl.
pub fn expbuf<R: FromRawFd>(
    fd: &impl AsRawFd,
    queue: QueueType,
    index: usize,
    plane: usize,
    flags: ExpbufFlags,
) -> Result<R, ExpbufError> {
    let mut v4l2_expbuf = v4l2_exportbuffer {
        type_: queue as u32,
        index: index as u32,
        plane: plane as u32,
        flags: flags.bits(),
        ..Default::default()
    };

    unsafe { ioctl::vidioc_expbuf(fd.as_raw_fd(), &mut v4l2_expbuf) }?;

    Ok(unsafe { R::from_raw_fd(v4l2_expbuf.fd) })
}
