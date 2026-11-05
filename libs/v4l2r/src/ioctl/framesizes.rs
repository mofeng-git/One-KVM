use nix::errno::Errno;
use std::os::unix::io::AsRawFd;
use thiserror::Error;

use crate::bindings;
use crate::bindings::v4l2_frmsizeenum;
use crate::PixelFormat;

/// A wrapper for the 'v4l2_frmsizeenum' union member types
#[derive(Debug)]
pub enum FrmSizeTypes<'a> {
    Discrete(&'a bindings::v4l2_frmsize_discrete),
    StepWise(&'a bindings::v4l2_frmsize_stepwise),
}

impl v4l2_frmsizeenum {
    /// Safely access the size member of the struct based on the
    /// returned type.
    pub fn size(&self) -> Option<FrmSizeTypes<'_>> {
        match self.type_ {
            // SAFETY: the member of the union that gets used by the driver
            // is determined by the type
            bindings::v4l2_frmsizetypes_V4L2_FRMSIZE_TYPE_DISCRETE => {
                Some(FrmSizeTypes::Discrete(unsafe {
                    &self.__bindgen_anon_1.discrete
                }))
            }

            // SAFETY: the member of the union that gets used by the driver
            // is determined by the type
            bindings::v4l2_frmsizetypes_V4L2_FRMSIZE_TYPE_CONTINUOUS
            | bindings::v4l2_frmsizetypes_V4L2_FRMSIZE_TYPE_STEPWISE => {
                Some(FrmSizeTypes::StepWise(unsafe {
                    &self.__bindgen_anon_1.stepwise
                }))
            }

            _ => None,
        }
    }
}

#[doc(hidden)]
mod ioctl {
    use crate::bindings::v4l2_frmsizeenum;
    nix::ioctl_readwrite!(vidioc_enum_framesizes, b'V', 74, v4l2_frmsizeenum);
}

#[derive(Debug, Error)]
pub enum FrameSizeError {
    #[error("Unexpected ioctl error: {0}")]
    IoctlError(nix::Error),
}

impl From<FrameSizeError> for Errno {
    fn from(err: FrameSizeError) -> Self {
        match err {
            FrameSizeError::IoctlError(e) => e,
        }
    }
}

/// Safe wrapper around the `VIDIOC_ENUM_FRAMESIZES` ioctl.
pub fn enum_frame_sizes<O: From<v4l2_frmsizeenum>>(
    fd: &impl AsRawFd,
    index: u32,
    pixel_format: PixelFormat,
) -> Result<O, FrameSizeError> {
    let mut frame_size = v4l2_frmsizeenum {
        index,
        pixel_format: pixel_format.into(),
        ..Default::default()
    };

    match unsafe { ioctl::vidioc_enum_framesizes(fd.as_raw_fd(), &mut frame_size) } {
        Ok(_) => Ok(O::from(frame_size)),
        Err(e) => Err(FrameSizeError::IoctlError(e)),
    }
}
