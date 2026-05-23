use nix::errno::Errno;
use std::os::unix::io::AsRawFd;
use thiserror::Error;

use crate::bindings;
use crate::bindings::v4l2_frmivalenum;
use crate::PixelFormat;

/// A wrapper for the 'v4l2_frmivalenum' union member types
#[derive(Debug)]
pub enum FrmIvalTypes<'a> {
    Discrete(&'a bindings::v4l2_fract),
    StepWise(&'a bindings::v4l2_frmival_stepwise),
}

impl v4l2_frmivalenum {
    /// Safely access the intervals member of the struct based on the
    /// returned type.
    pub fn intervals(&self) -> Option<FrmIvalTypes<'_>> {
        match self.type_ {
            // SAFETY: the member of the union that gets used by the driver
            // is determined by the type
            bindings::v4l2_frmivaltypes_V4L2_FRMIVAL_TYPE_DISCRETE => {
                Some(FrmIvalTypes::Discrete(unsafe {
                    &self.__bindgen_anon_1.discrete
                }))
            }

            // SAFETY: the member of the union that gets used by the driver
            // is determined by the type
            bindings::v4l2_frmivaltypes_V4L2_FRMIVAL_TYPE_CONTINUOUS
            | bindings::v4l2_frmivaltypes_V4L2_FRMIVAL_TYPE_STEPWISE => {
                Some(FrmIvalTypes::StepWise(unsafe {
                    &self.__bindgen_anon_1.stepwise
                }))
            }

            _ => None,
        }
    }
}

#[doc(hidden)]
mod ioctl {
    use crate::bindings::v4l2_frmivalenum;
    nix::ioctl_readwrite!(vidioc_enum_frameintervals, b'V', 75, v4l2_frmivalenum);
}

#[derive(Debug, Error)]
pub enum FrameIntervalsError {
    #[error("Unexpected ioctl error: {0}")]
    IoctlError(nix::Error),
}

impl From<FrameIntervalsError> for Errno {
    fn from(err: FrameIntervalsError) -> Self {
        match err {
            FrameIntervalsError::IoctlError(e) => e,
        }
    }
}
/// Safe wrapper around the `VIDIOC_ENUM_FRAMEINTERVALS` ioctl.
pub fn enum_frame_intervals<O: From<v4l2_frmivalenum>>(
    fd: &impl AsRawFd,
    index: u32,
    pixel_format: PixelFormat,
    width: u32,
    height: u32,
) -> Result<O, FrameIntervalsError> {
    let mut frame_interval = v4l2_frmivalenum {
        index,
        pixel_format: pixel_format.into(),
        width,
        height,
        ..Default::default()
    };

    match unsafe { ioctl::vidioc_enum_frameintervals(fd.as_raw_fd(), &mut frame_interval) } {
        Ok(_) => Ok(O::from(frame_interval)),
        Err(e) => Err(FrameIntervalsError::IoctlError(e)),
    }
}
