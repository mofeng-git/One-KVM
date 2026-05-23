//! Safe wrapper for the `VIDIOC_QUERYCAP` ioctl.
use super::string_from_cstr;
use crate::bindings;
use crate::bindings::v4l2_capability;
use bitflags::bitflags;
use nix::errno::Errno;
use std::fmt;
use std::os::unix::io::AsRawFd;
use thiserror::Error;

bitflags! {
    /// Flags returned by the `VIDIOC_QUERYCAP` ioctl into the `capabilities`
    /// or `device_capabilities` field of `v4l2_capability`.
    #[derive(Clone, Copy, Debug)]
    pub struct Capabilities: u32 {
        const VIDEO_CAPTURE = bindings::V4L2_CAP_VIDEO_CAPTURE;
        const VIDEO_OUTPUT = bindings::V4L2_CAP_VIDEO_OUTPUT;
        const VIDEO_OVERLAY = bindings::V4L2_CAP_VIDEO_OVERLAY;
        const VBI_CAPTURE = bindings::V4L2_CAP_VBI_CAPTURE;
        const VBI_OUTPUT = bindings::V4L2_CAP_VBI_OUTPUT;
        const SLICED_VBI_CAPTURE = bindings::V4L2_CAP_SLICED_VBI_CAPTURE;
        const SLICED_VBI_OUTPUT = bindings::V4L2_CAP_SLICED_VBI_OUTPUT;
        const RDS_CAPTURE = bindings::V4L2_CAP_RDS_CAPTURE;
        const VIDEO_OUTPUT_OVERLAY = bindings::V4L2_CAP_VIDEO_OUTPUT_OVERLAY;
        const HW_FREQ_SEEK = bindings::V4L2_CAP_HW_FREQ_SEEK;
        const RDS_OUTPUT = bindings::V4L2_CAP_RDS_OUTPUT;

        const VIDEO_CAPTURE_MPLANE = bindings::V4L2_CAP_VIDEO_CAPTURE_MPLANE;
        const VIDEO_OUTPUT_MPLANE = bindings::V4L2_CAP_VIDEO_OUTPUT_MPLANE;
        const VIDEO_M2M_MPLANE = bindings::V4L2_CAP_VIDEO_M2M_MPLANE;
        const VIDEO_M2M = bindings::V4L2_CAP_VIDEO_M2M;

        const TUNER = bindings::V4L2_CAP_TUNER;
        const AUDIO = bindings::V4L2_CAP_AUDIO;
        const RADIO = bindings::V4L2_CAP_RADIO;
        const MODULATOR = bindings::V4L2_CAP_MODULATOR;

        const SDR_CAPTURE = bindings::V4L2_CAP_SDR_CAPTURE;
        const EXT_PIX_FORMAT = bindings::V4L2_CAP_EXT_PIX_FORMAT;
        const SDR_OUTPUT = bindings::V4L2_CAP_SDR_OUTPUT;
        const META_CAPTURE = bindings::V4L2_CAP_META_CAPTURE;

        const READWRITE = bindings::V4L2_CAP_READWRITE;
        const ASYNCIO = bindings::V4L2_CAP_ASYNCIO;
        const STREAMING = bindings::V4L2_CAP_STREAMING;
        const META_OUTPUT = bindings::V4L2_CAP_META_OUTPUT;

        const TOUCH = bindings::V4L2_CAP_TOUCH;

        const DEVICE_CAPS = bindings::V4L2_CAP_DEVICE_CAPS;
    }
}

impl fmt::Display for Capabilities {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

/// Used to get the capability flags from a `VIDIOC_QUERYCAP` ioctl.
impl From<v4l2_capability> for Capabilities {
    fn from(qcap: v4l2_capability) -> Self {
        Capabilities::from_bits_truncate(qcap.capabilities)
    }
}

/// Safe variant of the `v4l2_capability` struct, to be used with `querycap`.
#[derive(Debug)]
pub struct Capability {
    pub driver: String,
    pub card: String,
    pub bus_info: String,
    pub version: u32,
    pub capabilities: Capabilities,
    pub device_caps: Option<Capabilities>,
}

impl Capability {
    /// Returns the set of capabilities of the hardware as a whole.
    pub fn capabilities(&self) -> Capabilities {
        self.capabilities
    }

    /// Returns the capabilities that apply to the currently opened V4L2 node.
    pub fn device_caps(&self) -> Capabilities {
        self.device_caps
            .unwrap_or_else(|| self.capabilities.difference(Capabilities::DEVICE_CAPS))
    }
}

impl From<v4l2_capability> for Capability {
    fn from(qcap: v4l2_capability) -> Self {
        Capability {
            driver: string_from_cstr(&qcap.driver).unwrap_or_else(|_| "".into()),
            card: string_from_cstr(&qcap.card).unwrap_or_else(|_| "".into()),
            bus_info: string_from_cstr(&qcap.bus_info).unwrap_or_else(|_| "".into()),
            version: qcap.version,
            capabilities: Capabilities::from_bits_truncate(qcap.capabilities),
            device_caps: if qcap.capabilities & bindings::V4L2_CAP_DEVICE_CAPS != 0 {
                Some(Capabilities::from_bits_truncate(qcap.device_caps))
            } else {
                None
            },
        }
    }
}

#[doc(hidden)]
mod ioctl {
    use crate::bindings::v4l2_capability;
    nix::ioctl_read!(vidioc_querycap, b'V', 0, v4l2_capability);
}

#[derive(Debug, Error)]
pub enum QueryCapError {
    #[error("ioctl error: {0}")]
    IoctlError(Errno),
}

impl From<QueryCapError> for Errno {
    fn from(err: QueryCapError) -> Self {
        match err {
            QueryCapError::IoctlError(e) => e,
        }
    }
}

/// Safe wrapper around the `VIDIOC_QUERYCAP` ioctl.
pub fn querycap<T: From<v4l2_capability>>(fd: &impl AsRawFd) -> Result<T, QueryCapError> {
    let mut qcap: v4l2_capability = Default::default();

    match unsafe { ioctl::vidioc_querycap(fd.as_raw_fd(), &mut qcap) } {
        Ok(_) => Ok(T::from(qcap)),
        Err(e) => Err(QueryCapError::IoctlError(e)),
    }
}
