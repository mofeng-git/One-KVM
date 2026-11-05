use std::os::unix::io::AsRawFd;

use nix::errno::Errno;
use thiserror::Error;

use crate::bindings::v4l2_standard;
use crate::bindings::v4l2_std_id;
use crate::bindings::v4l2_streamparm;
use crate::QueueType;

#[doc(hidden)]
mod ioctl {
    use crate::bindings::v4l2_standard;
    use crate::bindings::v4l2_std_id;
    use crate::bindings::v4l2_streamparm;

    nix::ioctl_readwrite!(vidioc_g_parm, b'V', 21, v4l2_streamparm);
    nix::ioctl_readwrite!(vidioc_s_parm, b'V', 22, v4l2_streamparm);
    nix::ioctl_read!(vidioc_g_std, b'V', 23, v4l2_std_id);
    nix::ioctl_write_ptr!(vidioc_s_std, b'V', 24, v4l2_std_id);
    nix::ioctl_readwrite!(vidioc_enumstd, b'V', 25, v4l2_standard);
    nix::ioctl_read!(vidioc_querystd, b'V', 63, v4l2_std_id);
}

#[derive(Debug, Error)]
pub enum GParmError {
    #[error("ioctl error: {0}")]
    IoctlError(Errno),
}

impl From<GParmError> for Errno {
    fn from(err: GParmError) -> Self {
        match err {
            GParmError::IoctlError(e) => e,
        }
    }
}

/// Safe wrapper around the `VIDIOC_G_PARM` ioctl.
pub fn g_parm<O: From<v4l2_streamparm>>(
    fd: &impl AsRawFd,
    queue: QueueType,
) -> Result<O, GParmError> {
    let mut parm = v4l2_streamparm {
        type_: queue as u32,
        ..Default::default()
    };

    match unsafe { ioctl::vidioc_g_parm(fd.as_raw_fd(), &mut parm) } {
        Ok(_) => Ok(O::from(parm)),
        Err(e) => Err(GParmError::IoctlError(e)),
    }
}

/// Safe wrapper around the `VIDIOC_S_PARM` ioctl.
pub fn s_parm<I: Into<v4l2_streamparm>, O: From<v4l2_streamparm>>(
    fd: &impl AsRawFd,
    parm: I,
) -> Result<O, GParmError> {
    let mut parm = parm.into();

    match unsafe { ioctl::vidioc_s_parm(fd.as_raw_fd(), &mut parm) } {
        Ok(_) => Ok(O::from(parm)),
        Err(e) => Err(GParmError::IoctlError(e)),
    }
}

/// Safe wrapper around the `VIDIOC_G_STD` ioctl.
pub fn g_std<O: From<v4l2_std_id>>(fd: &impl AsRawFd) -> Result<O, GParmError> {
    let mut std_id: v4l2_std_id = 0;

    match unsafe { ioctl::vidioc_g_std(fd.as_raw_fd(), &mut std_id) } {
        Ok(_) => Ok(O::from(std_id)),
        Err(e) => Err(GParmError::IoctlError(e)),
    }
}

#[derive(Debug, Error)]
pub enum SStdError {
    #[error("unsupported standard requested")]
    Unsupported,
    #[error("ioctl error: {0}")]
    IoctlError(Errno),
}

impl From<SStdError> for Errno {
    fn from(err: SStdError) -> Self {
        match err {
            SStdError::Unsupported => Errno::EINVAL,
            SStdError::IoctlError(e) => e,
        }
    }
}

/// Safe wrapper around the `VIDIOC_S_STD` ioctl.
pub fn s_std<I: Into<v4l2_std_id>>(fd: &impl AsRawFd, std_id: I) -> Result<(), SStdError> {
    let std_id = std_id.into();

    match unsafe { ioctl::vidioc_s_std(fd.as_raw_fd(), &std_id) } {
        Ok(_) => Ok(()),
        Err(Errno::EINVAL) => Err(SStdError::Unsupported),
        Err(e) => Err(SStdError::IoctlError(e)),
    }
}

#[derive(Debug, Error)]
pub enum EnumStdError {
    #[error("requested index is out of bounds")]
    OutOfBounds,
    #[error("standard video timings are not supported for this input or output")]
    Unsupported,
    #[error("ioctl error: {0}")]
    IoctlError(Errno),
}

impl From<EnumStdError> for Errno {
    fn from(err: EnumStdError) -> Self {
        match err {
            EnumStdError::OutOfBounds => Errno::EINVAL,
            EnumStdError::Unsupported => Errno::ENODATA,
            EnumStdError::IoctlError(e) => e,
        }
    }
}

/// Safe wrapper around the `VIDIOC_ENUMSTD` ioctl.
pub fn enumstd<O: From<v4l2_standard>>(fd: &impl AsRawFd, index: u32) -> Result<O, EnumStdError> {
    let mut standard = v4l2_standard {
        index,
        ..Default::default()
    };

    match unsafe { ioctl::vidioc_enumstd(fd.as_raw_fd(), &mut standard) } {
        Ok(_) => Ok(O::from(standard)),
        Err(Errno::EINVAL) => Err(EnumStdError::OutOfBounds),
        Err(Errno::ENODATA) => Err(EnumStdError::Unsupported),
        Err(e) => Err(EnumStdError::IoctlError(e)),
    }
}

/// Safe wrapper around the `VIDIOC_QUERYSTD` ioctl.
pub fn querystd<O: From<v4l2_std_id>>(fd: &impl AsRawFd) -> Result<O, GParmError> {
    let mut std_id: v4l2_std_id = 0;

    match unsafe { ioctl::vidioc_querystd(fd.as_raw_fd(), &mut std_id) } {
        Ok(_) => Ok(O::from(std_id)),
        Err(e) => Err(GParmError::IoctlError(e)),
    }
}
