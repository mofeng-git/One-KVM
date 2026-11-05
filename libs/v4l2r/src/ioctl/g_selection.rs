use std::os::unix::io::AsRawFd;

use bitflags::bitflags;
use enumn::N;
use nix::errno::Errno;
use thiserror::Error;

use crate::bindings;
use crate::bindings::v4l2_rect;
use crate::bindings::v4l2_selection;

#[derive(Debug, N, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum SelectionType {
    Capture = bindings::v4l2_buf_type_V4L2_BUF_TYPE_VIDEO_CAPTURE,
    Output = bindings::v4l2_buf_type_V4L2_BUF_TYPE_VIDEO_OUTPUT,
}

#[derive(Debug, N, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum SelectionTarget {
    Crop = bindings::V4L2_SEL_TGT_CROP,
    CropDefault = bindings::V4L2_SEL_TGT_CROP_DEFAULT,
    CropBounds = bindings::V4L2_SEL_TGT_CROP_BOUNDS,
    NativeSize = bindings::V4L2_SEL_TGT_NATIVE_SIZE,
    Compose = bindings::V4L2_SEL_TGT_COMPOSE,
    ComposeDefault = bindings::V4L2_SEL_TGT_COMPOSE_DEFAULT,
    ComposeBounds = bindings::V4L2_SEL_TGT_COMPOSE_BOUNDS,
    ComposePadded = bindings::V4L2_SEL_TGT_COMPOSE_PADDED,
}

bitflags! {
    #[derive(Clone, Copy, Debug)]
    pub struct SelectionFlags: u32 {
        const GE = bindings::V4L2_SEL_FLAG_GE;
        const LE = bindings::V4L2_SEL_FLAG_LE;
        const KEEP_CONFIG = bindings::V4L2_SEL_FLAG_KEEP_CONFIG;
    }
}

#[doc(hidden)]
mod ioctl {
    use crate::bindings::v4l2_selection;
    nix::ioctl_readwrite!(vidioc_g_selection, b'V', 94, v4l2_selection);
    nix::ioctl_readwrite!(vidioc_s_selection, b'V', 95, v4l2_selection);
}

#[derive(Debug, Error)]
pub enum GSelectionError {
    #[error("invalid type or target requested")]
    Invalid,
    #[error("ioctl error: {0}")]
    IoctlError(Errno),
}

impl From<GSelectionError> for Errno {
    fn from(err: GSelectionError) -> Self {
        match err {
            GSelectionError::Invalid => Errno::EINVAL,
            GSelectionError::IoctlError(e) => e,
        }
    }
}

/// Safe wrapper around the `VIDIOC_G_SELECTION` ioctl.
pub fn g_selection<R: From<v4l2_rect>>(
    fd: &impl AsRawFd,
    selection: SelectionType,
    target: SelectionTarget,
) -> Result<R, GSelectionError> {
    let mut sel = v4l2_selection {
        type_: selection as u32,
        target: target as u32,
        ..Default::default()
    };

    match unsafe { ioctl::vidioc_g_selection(fd.as_raw_fd(), &mut sel) } {
        Ok(_) => Ok(R::from(sel.r)),
        Err(Errno::EINVAL) => Err(GSelectionError::Invalid),
        Err(e) => Err(GSelectionError::IoctlError(e)),
    }
}

#[derive(Debug, Error)]
pub enum SSelectionError {
    #[error("invalid type or target requested")]
    Invalid,
    #[error("invalid range requested")]
    InvalidRange,
    #[error("cannot change selection rectangle currently")]
    Busy,
    #[error("ioctl error: {0}")]
    IoctlError(nix::Error),
}

impl From<SSelectionError> for Errno {
    fn from(err: SSelectionError) -> Self {
        match err {
            SSelectionError::Invalid => Errno::EINVAL,
            SSelectionError::InvalidRange => Errno::ERANGE,
            SSelectionError::Busy => Errno::EBUSY,
            SSelectionError::IoctlError(e) => e,
        }
    }
}

/// Safe wrapper around the `VIDIOC_S_SELECTION` ioctl.
pub fn s_selection<RI: Into<v4l2_rect>, RO: From<v4l2_rect>>(
    fd: &impl AsRawFd,
    selection: SelectionType,
    target: SelectionTarget,
    rect: RI,
    flags: SelectionFlags,
) -> Result<RO, SSelectionError> {
    let mut sel = v4l2_selection {
        type_: selection as u32,
        target: target as u32,
        flags: flags.bits(),
        r: rect.into(),
        ..Default::default()
    };

    match unsafe { ioctl::vidioc_s_selection(fd.as_raw_fd(), &mut sel) } {
        Ok(_) => Ok(RO::from(sel.r)),
        Err(Errno::EINVAL) => Err(SSelectionError::Invalid),
        Err(Errno::ERANGE) => Err(SSelectionError::InvalidRange),
        Err(Errno::EBUSY) => Err(SSelectionError::Busy),
        Err(e) => Err(SSelectionError::IoctlError(e)),
    }
}
