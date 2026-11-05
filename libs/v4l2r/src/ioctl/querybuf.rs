use std::convert::Infallible;
use std::convert::TryFrom;
use std::os::unix::io::AsRawFd;

use nix::errno::Errno;
use thiserror::Error;

use crate::ioctl::ioctl_and_convert;
use crate::ioctl::BufferFlags;
use crate::ioctl::IoctlConvertError;
use crate::ioctl::IoctlConvertResult;
use crate::ioctl::UncheckedV4l2Buffer;
use crate::QueueType;

#[derive(Debug)]
pub struct QueryBufPlane {
    /// Offset to pass to `mmap()` in order to obtain a mapping for this plane.
    pub mem_offset: u32,
    /// Length of this plane.
    pub length: u32,
}

/// Contains information about a buffer's layout, as obtained from [`crate::ioctl::querybuf`].
///
/// It is a subset of [`crate::ioctl::V4l2Buffer`], only more convenient on occasion because its
/// conversion from an unchecked v4l2_buffer cannot fail.
///
/// Single-planar buffers have one entry in [`planes`] representing the layout of their unique
/// plane.
#[derive(Debug)]
pub struct QueryBuffer {
    pub index: usize,
    pub flags: BufferFlags,
    pub planes: Vec<QueryBufPlane>,
}

impl TryFrom<UncheckedV4l2Buffer> for QueryBuffer {
    type Error = Infallible;

    fn try_from(buffer: UncheckedV4l2Buffer) -> Result<Self, Self::Error> {
        let v4l2_buf = buffer.0;
        let planes = match buffer.1 {
            None => vec![QueryBufPlane {
                mem_offset: unsafe { v4l2_buf.m.offset },
                length: v4l2_buf.length,
            }],
            Some(v4l2_planes) => v4l2_planes
                .iter()
                .take(v4l2_buf.length as usize)
                .map(|v4l2_plane| QueryBufPlane {
                    mem_offset: unsafe { v4l2_plane.m.mem_offset },
                    length: v4l2_plane.length,
                })
                .collect(),
        };

        Ok(QueryBuffer {
            index: v4l2_buf.index as usize,
            flags: BufferFlags::from_bits_truncate(v4l2_buf.flags),
            planes,
        })
    }
}

#[doc(hidden)]
mod ioctl {
    use crate::bindings::v4l2_buffer;
    nix::ioctl_readwrite!(vidioc_querybuf, b'V', 9, v4l2_buffer);
}

#[derive(Debug, Error)]
pub enum QueryBufIoctlError {
    #[error("unsupported queue or out-of-bounds index")]
    InvalidInput,
    #[error("unexpected ioctl error: {0}")]
    Other(Errno),
}

impl From<Errno> for QueryBufIoctlError {
    fn from(err: Errno) -> Self {
        match err {
            Errno::EINVAL => QueryBufIoctlError::InvalidInput,
            e => QueryBufIoctlError::Other(e),
        }
    }
}

impl From<QueryBufIoctlError> for Errno {
    fn from(err: QueryBufIoctlError) -> Self {
        match err {
            QueryBufIoctlError::InvalidInput => Errno::EINVAL,
            QueryBufIoctlError::Other(e) => e,
        }
    }
}

pub type QueryBufError<CE> = IoctlConvertError<QueryBufIoctlError, CE>;
pub type QueryBufResult<O, CE> = IoctlConvertResult<O, QueryBufIoctlError, CE>;

/// Safe wrapper around the `VIDIOC_QUERYBUF` ioctl.
pub fn querybuf<O>(fd: &impl AsRawFd, queue: QueueType, index: usize) -> QueryBufResult<O, O::Error>
where
    O: TryFrom<UncheckedV4l2Buffer>,
    O::Error: std::fmt::Debug,
{
    let mut v4l2_buf = UncheckedV4l2Buffer::new_for_querybuf(queue, Some(index as u32));

    ioctl_and_convert(
        unsafe { ioctl::vidioc_querybuf(fd.as_raw_fd(), v4l2_buf.as_mut()) }
            .map(|_| v4l2_buf)
            .map_err(Into::into),
    )
}
