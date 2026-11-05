//! Safe wrapper for the `VIDIOC_ENUM_FMT` ioctl.
use super::string_from_cstr;
use crate::bindings;
use crate::bindings::v4l2_fmtdesc;
use crate::{PixelFormat, QueueType};
use bitflags::bitflags;
use log::error;
use nix::errno::Errno;
use std::fmt;
use std::os::unix::io::AsRawFd;
use thiserror::Error;

bitflags! {
    /// Flags returned by the `VIDIOC_ENUM_FMT` ioctl into the `flags` field of
    /// `struct v4l2_fmtdesc`.
    #[derive(Clone, Copy, Debug)]
    pub struct FormatFlags: u32 {
        const COMPRESSED = bindings::V4L2_FMT_FLAG_COMPRESSED;
        const EMULATED = bindings::V4L2_FMT_FLAG_EMULATED;
    }
}
/// Quickly get the Fourcc code of a format.
impl From<v4l2_fmtdesc> for PixelFormat {
    fn from(fmtdesc: v4l2_fmtdesc) -> Self {
        fmtdesc.pixelformat.into()
    }
}

/// Safe variant of the `v4l2_fmtdesc` struct, to be used with `enum_fmt`.
#[derive(Debug)]
pub struct FmtDesc {
    pub flags: FormatFlags,
    pub description: String,
    pub pixelformat: PixelFormat,
}

impl fmt::Display for FmtDesc {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}: {} {}",
            self.pixelformat,
            self.description,
            if self.flags.is_empty() {
                "".into()
            } else {
                format!("({:?})", self.flags)
            }
        )
    }
}

impl From<v4l2_fmtdesc> for FmtDesc {
    fn from(fmtdesc: v4l2_fmtdesc) -> Self {
        FmtDesc {
            flags: FormatFlags::from_bits_truncate(fmtdesc.flags),
            description: string_from_cstr(&fmtdesc.description).unwrap_or_else(|_| "".into()),
            pixelformat: fmtdesc.pixelformat.into(),
        }
    }
}

#[doc(hidden)]
mod ioctl {
    use crate::bindings::v4l2_fmtdesc;
    nix::ioctl_readwrite!(vidioc_enum_fmt, b'V', 2, v4l2_fmtdesc);
}

#[derive(Debug, Error)]
pub enum EnumFmtError {
    #[error("ioctl error: {0}")]
    IoctlError(#[from] nix::Error),
}

impl From<EnumFmtError> for Errno {
    fn from(err: EnumFmtError) -> Self {
        match err {
            EnumFmtError::IoctlError(e) => e,
        }
    }
}

/// Safe wrapper around the `VIDIOC_ENUM_FMT` ioctl.
pub fn enum_fmt<T: From<v4l2_fmtdesc>>(
    fd: &impl AsRawFd,
    queue: QueueType,
    index: u32,
) -> Result<T, EnumFmtError> {
    let mut fmtdesc = v4l2_fmtdesc {
        type_: queue as u32,
        index,
        ..Default::default()
    };
    unsafe { ioctl::vidioc_enum_fmt(fd.as_raw_fd(), &mut fmtdesc) }?;

    Ok(T::from(fmtdesc))
}

/// Iterator over the formats of the given queue. This takes a reference to the
/// device's file descriptor so no operation that could affect the format
/// enumeration can take place while the iterator exists.
pub struct FormatIterator<'a, F: AsRawFd> {
    fd: &'a F,
    queue: QueueType,
    index: u32,
}

impl<'a, F: AsRawFd> FormatIterator<'a, F> {
    /// Create a new iterator listing all the currently valid formats on
    /// `queue`.
    pub fn new(fd: &'a F, queue: QueueType) -> Self {
        FormatIterator {
            fd,
            queue,
            index: 0,
        }
    }
}

impl<'a, F: AsRawFd> Iterator for FormatIterator<'a, F> {
    type Item = FmtDesc;

    fn next(&mut self) -> Option<Self::Item> {
        match enum_fmt(self.fd, self.queue, self.index) {
            Ok(fmtdesc) => {
                self.index += 1;
                Some(fmtdesc)
            }
            // EINVAL means we have reached the last format.
            Err(EnumFmtError::IoctlError(Errno::EINVAL)) => None,
            _ => {
                error!("Unexpected return value for VIDIOC_ENUM_FMT!");
                None
            }
        }
    }
}
