//! Shared tuning for V4L2 MJPEG capture paths (`Streamer` + `SharedVideoPipeline`).

/// Frames smaller than this are treated as incomplete / noise.
pub(crate) const MIN_CAPTURE_FRAME_SIZE: usize = 128;

/// After startup, validate JPEG header every N frames to limit CPU use.
pub(crate) const JPEG_VALIDATE_INTERVAL: u64 = 30;

/// Validate every MJPEG frame for the first N frames (UVC warm-up / bad headers).
pub(crate) const STARTUP_JPEG_VALIDATE_FRAMES: u64 = 3;

#[inline]
pub(crate) fn should_validate_jpeg_frame(validate_counter: u64) -> bool {
    validate_counter <= STARTUP_JPEG_VALIDATE_FRAMES
        || validate_counter.is_multiple_of(JPEG_VALIDATE_INTERVAL)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jpeg_validation_policy_startup_then_interval() {
        assert!(should_validate_jpeg_frame(1));
        assert!(should_validate_jpeg_frame(2));
        assert!(should_validate_jpeg_frame(3));
        assert!(!should_validate_jpeg_frame(4));
        assert!(should_validate_jpeg_frame(30));
    }
}
