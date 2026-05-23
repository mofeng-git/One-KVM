use crate::ioctl::ioctl_and_convert;
use crate::ioctl::IoctlConvertError;
use crate::ioctl::IoctlConvertResult;
use crate::ioctl::UncheckedV4l2Buffer;
use crate::QueueType;

use std::convert::TryFrom;
use std::fmt::Debug;
use std::os::unix::io::AsRawFd;

use nix::errno::Errno;
use thiserror::Error;

#[doc(hidden)]
mod ioctl {
    use crate::bindings::v4l2_buffer;
    nix::ioctl_readwrite!(vidioc_dqbuf, b'V', 17, v4l2_buffer);
}

#[derive(Debug, Error)]
pub enum DqBufIoctlError {
    #[error("end-of-stream reached")]
    Eos,
    #[error("no buffer ready for dequeue")]
    NotReady,
    #[error("unexpected ioctl error: {0}")]
    Other(Errno),
}

impl From<Errno> for DqBufIoctlError {
    fn from(error: Errno) -> Self {
        match error {
            Errno::EAGAIN => Self::NotReady,
            Errno::EPIPE => Self::Eos,
            error => Self::Other(error),
        }
    }
}

impl From<DqBufIoctlError> for Errno {
    fn from(err: DqBufIoctlError) -> Self {
        match err {
            DqBufIoctlError::Eos => Errno::EPIPE,
            DqBufIoctlError::NotReady => Errno::EAGAIN,
            DqBufIoctlError::Other(e) => e,
        }
    }
}

pub type DqBufError<CE> = IoctlConvertError<DqBufIoctlError, CE>;
pub type DqBufResult<O, CE> = IoctlConvertResult<O, DqBufIoctlError, CE>;

/// Safe wrapper around the `VIDIOC_DQBUF` ioctl.
pub fn dqbuf<O>(fd: &impl AsRawFd, queue: QueueType) -> DqBufResult<O, O::Error>
where
    O: TryFrom<UncheckedV4l2Buffer>,
    O::Error: std::fmt::Debug,
{
    let mut v4l2_buf = UncheckedV4l2Buffer::new_for_querybuf(queue, None);

    ioctl_and_convert(
        unsafe { ioctl::vidioc_dqbuf(fd.as_raw_fd(), v4l2_buf.as_mut()) }
            .map(|_| v4l2_buf)
            .map_err(Into::into),
    )
}
