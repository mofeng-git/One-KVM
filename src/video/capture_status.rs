//! Shared capture status and error classification helpers.

use std::io;

use crate::video::SignalStatus;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureIoErrorKind {
    DeviceLost,
    TransientSignal { status: Option<SignalStatus> },
    Other,
}

pub fn signal_status_from_capture_kind(kind: &str) -> SignalStatus {
    SignalStatus::from_str(kind).unwrap_or(SignalStatus::NoSignal)
}

pub fn classify_capture_io_error(err: &io::Error) -> CaptureIoErrorKind {
    match err.raw_os_error() {
        // ENXIO / ENODEV / ESHUTDOWN: the device node or endpoint is gone.
        Some(6) | Some(19) | Some(108) => CaptureIoErrorKind::DeviceLost,
        // EIO / EPIPE: source or transport glitched; EPROTO is common for UVC USB.
        Some(5) | Some(32) => CaptureIoErrorKind::TransientSignal { status: None },
        Some(71) => CaptureIoErrorKind::TransientSignal {
            status: Some(SignalStatus::UvcUsbError),
        },
        _ => CaptureIoErrorKind::Other,
    }
}

pub fn capture_error_log_key(err: &io::Error) -> String {
    let message = err.to_string();
    if message.contains("dqbuf failed") && message.contains("EINVAL") {
        "capture_dqbuf_einval".to_string()
    } else if message.contains("dqbuf failed") {
        "capture_dqbuf".to_string()
    } else {
        format!("capture_{:?}", err.kind())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_known_signal_status_strings() {
        assert_eq!(
            signal_status_from_capture_kind("out_of_range"),
            SignalStatus::OutOfRange
        );
        assert_eq!(
            signal_status_from_capture_kind("unknown"),
            SignalStatus::NoSignal
        );
    }

    #[test]
    fn classifies_source_change_log_keys() {
        let err = io::Error::other("dqbuf failed: EINVAL");
        assert_eq!(capture_error_log_key(&err), "capture_dqbuf_einval");

        let err = io::Error::new(io::ErrorKind::TimedOut, "capture timeout");
        assert_eq!(capture_error_log_key(&err), "capture_TimedOut");
    }
}
