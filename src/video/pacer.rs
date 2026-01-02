//! Encoder Pacer - Placeholder for future backpressure control
//!
//! Currently a pass-through that allows all frames.
//! TODO: Implement effective backpressure control.

use std::sync::atomic::{AtomicU64, Ordering};
use tracing::debug;

/// Encoder pacing statistics
#[derive(Debug, Clone, Default)]
pub struct PacerStats {
    /// Total frames processed
    pub frames_processed: u64,
    /// Frames skipped (currently always 0)
    pub frames_skipped: u64,
    /// Keyframes processed
    pub keyframes_processed: u64,
}

/// Encoder pacer (currently pass-through)
///
/// This is a placeholder for future backpressure control.
/// Currently allows all frames through without throttling.
pub struct EncoderPacer {
    frames_processed: AtomicU64,
    keyframes_processed: AtomicU64,
}

impl EncoderPacer {
    /// Create a new encoder pacer
    pub fn new(_max_in_flight: usize) -> Self {
        debug!("Creating encoder pacer (pass-through mode)");
        Self {
            frames_processed: AtomicU64::new(0),
            keyframes_processed: AtomicU64::new(0),
        }
    }

    /// Check if encoding should proceed (always returns true)
    pub async fn should_encode(&self, is_keyframe: bool) -> bool {
        self.frames_processed.fetch_add(1, Ordering::Relaxed);
        if is_keyframe {
            self.keyframes_processed.fetch_add(1, Ordering::Relaxed);
        }
        true // Always allow encoding
    }

    /// Report lag from receiver (currently no-op)
    pub async fn report_lag(&self, _frames_lagged: u64) {
        // TODO: Implement effective backpressure control
        // Currently this is a no-op
    }

    /// Check if throttling (always false)
    pub fn is_throttling(&self) -> bool {
        false
    }

    /// Get pacer statistics
    pub fn stats(&self) -> PacerStats {
        PacerStats {
            frames_processed: self.frames_processed.load(Ordering::Relaxed),
            frames_skipped: 0,
            keyframes_processed: self.keyframes_processed.load(Ordering::Relaxed),
        }
    }

    /// Get in-flight count (always 0)
    pub fn in_flight(&self) -> usize {
        0
    }
}
