//! Video signal status classification.

/// Fine-grained signal status reported by CSI/HDMI bridge devices.
///
/// Only `rk_hdmirx` / `rkcif` / tc358743-class bridges can distinguish these
/// via `VIDIOC_QUERY_DV_TIMINGS` errno; USB UVC devices always report `Ok`
/// until they fail with a generic timeout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalStatus {
    /// HDMI cable physically disconnected (`ENOLINK`).
    NoCable,
    /// TMDS signal present but timings cannot be locked (`ENOLCK`).
    NoSync,
    /// Timings outside of hardware capability (`ERANGE`).
    OutOfRange,
    /// Generic "no usable source" (fallback for EINVAL / EIO / unknown errnos).
    NoSignal,
    /// UVC/USB isochronous protocol error (common kernel: status -71 / userspace EPROTO).
    UvcUsbError,
    /// UVC capture stalled (repeated DQBUF timeouts; often cable, hub, or controller load).
    UvcCaptureStall,
}

impl SignalStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            SignalStatus::NoCable => "no_cable",
            SignalStatus::NoSync => "no_sync",
            SignalStatus::OutOfRange => "out_of_range",
            SignalStatus::NoSignal => "no_signal",
            SignalStatus::UvcUsbError => "uvc_usb_error",
            SignalStatus::UvcCaptureStall => "uvc_capture_stall",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        Some(match s {
            "no_cable" => SignalStatus::NoCable,
            "no_sync" => SignalStatus::NoSync,
            "out_of_range" => SignalStatus::OutOfRange,
            "no_signal" => SignalStatus::NoSignal,
            "uvc_usb_error" => SignalStatus::UvcUsbError,
            "uvc_capture_stall" => SignalStatus::UvcCaptureStall,
            _ => return None,
        })
    }
}
