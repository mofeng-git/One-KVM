use std::os::unix::io::AsRawFd;

use enumn::N;
use nix::errno::Errno;
use thiserror::Error;

use crate::bindings;
use crate::bindings::v4l2_dv_timings;
use crate::bindings::v4l2_dv_timings_cap;
use crate::bindings::v4l2_enum_dv_timings;

#[doc(hidden)]
mod ioctl {
    use crate::bindings::v4l2_dv_timings;
    use crate::bindings::v4l2_dv_timings_cap;
    use crate::bindings::v4l2_enum_dv_timings;

    nix::ioctl_readwrite!(vidioc_s_dv_timings, b'V', 87, v4l2_dv_timings);
    nix::ioctl_readwrite!(vidioc_g_dv_timings, b'V', 88, v4l2_dv_timings);
    nix::ioctl_readwrite!(vidioc_enum_dv_timings, b'V', 98, v4l2_enum_dv_timings);
    nix::ioctl_read!(vidioc_query_dv_timings, b'V', 99, v4l2_dv_timings);
    nix::ioctl_readwrite!(vidioc_dv_timings_cap, b'V', 100, v4l2_dv_timings_cap);
}

#[derive(Debug, N)]
#[repr(u32)]
pub enum DvTimingsType {
    Bt6561120 = bindings::V4L2_DV_BT_656_1120,
}

#[derive(Debug, Error)]
pub enum GDvTimingsError {
    #[error("ioctl not supported or invalid parameters")]
    Invalid,
    #[error("Digital video timings are not supported on this input or output")]
    Unsupported,
    #[error("Device is busy and cannot change timings")]
    Busy,
    #[error("ioctl error: {0}")]
    IoctlError(Errno),
}

impl From<GDvTimingsError> for Errno {
    fn from(err: GDvTimingsError) -> Self {
        match err {
            GDvTimingsError::Invalid => Errno::EINVAL,
            GDvTimingsError::Unsupported => Errno::ENODATA,
            GDvTimingsError::Busy => Errno::EBUSY,
            GDvTimingsError::IoctlError(e) => e,
        }
    }
}

/// Safe wrapper around the `VIDIOC_S_DV_TIMINGS` ioctl.
pub fn s_dv_timings<I: Into<v4l2_dv_timings>, O: From<v4l2_dv_timings>>(
    fd: &impl AsRawFd,
    timings: I,
) -> Result<O, GDvTimingsError> {
    let mut timings: v4l2_dv_timings = timings.into();

    match unsafe { ioctl::vidioc_s_dv_timings(fd.as_raw_fd(), &mut timings) } {
        Ok(_) => Ok(O::from(timings)),
        Err(Errno::EINVAL) => Err(GDvTimingsError::Invalid),
        Err(Errno::ENODATA) => Err(GDvTimingsError::Unsupported),
        Err(Errno::EBUSY) => Err(GDvTimingsError::Busy),
        Err(e) => Err(GDvTimingsError::IoctlError(e)),
    }
}

/// Safe wrapper around the `VIDIOC_G_DV_TIMINGS` ioctl.
pub fn g_dv_timings<O: From<v4l2_dv_timings>>(fd: &impl AsRawFd) -> Result<O, GDvTimingsError> {
    let mut timings = v4l2_dv_timings {
        ..Default::default()
    };

    match unsafe { ioctl::vidioc_g_dv_timings(fd.as_raw_fd(), &mut timings) } {
        Ok(_) => Ok(O::from(timings)),
        Err(Errno::EINVAL) => Err(GDvTimingsError::Invalid),
        Err(Errno::ENODATA) => Err(GDvTimingsError::Unsupported),
        Err(Errno::EBUSY) => Err(GDvTimingsError::Busy),
        Err(e) => Err(GDvTimingsError::IoctlError(e)),
    }
}

#[derive(Debug, Error)]
pub enum EnumDvTimingsError {
    #[error("timing index is out of bounds")]
    Invalid,
    #[error("Digital video timings are not supported on this input or output")]
    Unsupported,
    #[error("ioctl error: {0}")]
    IoctlError(Errno),
}

impl From<EnumDvTimingsError> for Errno {
    fn from(err: EnumDvTimingsError) -> Self {
        match err {
            EnumDvTimingsError::Invalid => Errno::EINVAL,
            EnumDvTimingsError::Unsupported => Errno::ENODATA,
            EnumDvTimingsError::IoctlError(e) => e,
        }
    }
}

/// Safe wrapper around the `VIDIOC_ENUM_DV_TIMINGS` ioctl.
pub fn enum_dv_timings<O: From<v4l2_dv_timings>>(
    fd: &impl AsRawFd,
    index: u32,
) -> Result<O, EnumDvTimingsError> {
    let mut timings = v4l2_enum_dv_timings {
        index,
        ..Default::default()
    };

    match unsafe { ioctl::vidioc_enum_dv_timings(fd.as_raw_fd(), &mut timings) } {
        Ok(_) => Ok(O::from(timings.timings)),
        Err(Errno::EINVAL) => Err(EnumDvTimingsError::Invalid),
        Err(Errno::ENODATA) => Err(EnumDvTimingsError::Unsupported),
        Err(e) => Err(EnumDvTimingsError::IoctlError(e)),
    }
}

#[derive(Debug, Error)]
pub enum QueryDvTimingsError {
    #[error("Digital video timings are not supported on this input or output")]
    Unsupported,
    #[error("No timings could be detected because no signal was found")]
    NoLink,
    #[error("Unstable signal")]
    UnstableSignal,
    #[error("ioctl error: {0}")]
    IoctlError(Errno),
}

impl From<QueryDvTimingsError> for Errno {
    fn from(err: QueryDvTimingsError) -> Self {
        match err {
            QueryDvTimingsError::Unsupported => Errno::ENODATA,
            QueryDvTimingsError::NoLink => Errno::ENOLINK,
            QueryDvTimingsError::UnstableSignal => Errno::ENOLCK,
            QueryDvTimingsError::IoctlError(e) => e,
        }
    }
}

/// Safe wrapper around the `VIDIOC_QUERY_DV_TIMINGS` ioctl.
pub fn query_dv_timings<O: From<v4l2_dv_timings>>(
    fd: &impl AsRawFd,
) -> Result<O, QueryDvTimingsError> {
    let mut timings = v4l2_dv_timings {
        ..Default::default()
    };

    match unsafe { ioctl::vidioc_query_dv_timings(fd.as_raw_fd(), &mut timings) } {
        Ok(_) => Ok(O::from(timings)),
        Err(Errno::ENODATA) => Err(QueryDvTimingsError::Unsupported),
        Err(Errno::ENOLINK) => Err(QueryDvTimingsError::NoLink),
        Err(Errno::ENOLCK) => Err(QueryDvTimingsError::UnstableSignal),
        Err(e) => Err(QueryDvTimingsError::IoctlError(e)),
    }
}

#[derive(Debug, Error)]
pub enum DvTimingsCapError {
    #[error("ioctl error: {0}")]
    IoctlError(Errno),
}

impl From<DvTimingsCapError> for Errno {
    fn from(err: DvTimingsCapError) -> Self {
        match err {
            DvTimingsCapError::IoctlError(e) => e,
        }
    }
}

/// Safe wrapper around the `VIDIOC_DV_TIMINGS_CAP` ioctl.
pub fn dv_timings_cap<O: From<v4l2_dv_timings_cap>>(
    fd: &impl AsRawFd,
) -> Result<O, DvTimingsCapError> {
    let mut caps = v4l2_dv_timings_cap {
        ..Default::default()
    };

    match unsafe { ioctl::vidioc_dv_timings_cap(fd.as_raw_fd(), &mut caps) } {
        Ok(_) => Ok(O::from(caps)),
        Err(e) => Err(DvTimingsCapError::IoctlError(e)),
    }
}
