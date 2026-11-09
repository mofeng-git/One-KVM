//! Progress and throughput statistics shared by sender & receiver.

use crate::time::now_ms;
use std::collections::VecDeque;
use std::time::Duration;

/// Length of the sliding window used for live FPS / throughput (ms).
///
/// The cumulative-average FPS (`frames / total_elapsed`) permanently decays
/// whenever the render loop ever stalls — background-tab throttling, a GC
/// pause, a dropped frame, device sleep — because `elapsed_ms` keeps growing
/// while `frames` does not. Over a long session those transient stalls drag the
/// displayed rate down even though the *current* output speed is unchanged.
/// Reporting the rate over only the last few seconds makes the numbers track
/// the live output instead of the whole-session average.
const STATS_WINDOW_MS: u64 = 3000;
/// Hard cap on buffered samples (guards against unbounded growth at very high
/// frame rates; 240 fps × 3 s = 720, so this is generous headroom).
const STATS_MAX_SAMPLES: usize = 4096;

/// One rate sample: the payload bytes counted at a wall-clock timestamp.
#[derive(Debug, Clone, Copy)]
struct RateSample {
    t_ms: u64,
    bytes: u64,
}

/// Live throughput / count snapshot.
#[derive(Debug, Clone, Default)]
pub struct Stats {
    /// Total bytes (payload only) sent or received so far.
    pub bytes: u64,
    /// Number of frames sent or received.
    pub frames: u64,
    /// Elapsed wall-clock since session start (ms), filled on snapshot.
    pub elapsed_ms: u64,
    /// Sliding window of recent frame samples for live rate reporting.
    window: VecDeque<RateSample>,
}

impl Stats {
    pub fn record_sent(&mut self, payload_bytes: u64) {
        self.bytes += payload_bytes;
        self.frames += 1;
        self.push_sample_at(now_ms(), payload_bytes);
    }

    pub fn record_received(&mut self, payload_bytes: u64, unique: bool) {
        // Only count unique symbols toward byte totals (dedup).
        let counted = if unique { payload_bytes } else { 0 };
        if unique {
            self.bytes += payload_bytes;
        }
        self.frames += 1;
        // Every received frame (unique or duplicate) occupies the channel, so it
        // contributes to the frame rate; only unique bytes count toward byte rate.
        self.push_sample_at(now_ms(), counted);
    }

    /// Recompute derived fields (call before exposing).
    pub fn finalize(&mut self) {}

    /// Effective frames per second, averaged over the last
    /// [`STATS_WINDOW_MS`] via a sliding window. Falls back to the whole-session
    /// cumulative average only while the window holds too few samples to be
    /// meaningful (the first moments of a transfer), so early numbers stay sane.
    pub fn fps(&self) -> f64 {
        self.fps_at(now_ms())
    }

    /// Effective payload throughput in bytes/sec, over the same sliding window.
    pub fn throughput_bps(&self) -> f64 {
        self.throughput_bps_at(now_ms())
    }

    /// `fps()` at an explicit `now` (for deterministic tests).
    fn fps_at(&self, now: u64) -> f64 {
        let (span_ms, frames, _) = self.window_rates(now);
        if frames < 2 || span_ms == 0 {
            self.cumulative_fps()
        } else {
            frames as f64 * 1000.0 / span_ms as f64
        }
    }

    /// `throughput_bps()` at an explicit `now` (for deterministic tests).
    fn throughput_bps_at(&self, now: u64) -> f64 {
        let (span_ms, _, bytes) = self.window_rates(now);
        if bytes == 0 || span_ms == 0 {
            self.cumulative_throughput_bps()
        } else {
            bytes as f64 * 1000.0 / span_ms as f64
        }
    }

    /// Whole-session cumulative FPS (pre-window behaviour, fallback).
    fn cumulative_fps(&self) -> f64 {
        if self.elapsed_ms == 0 {
            0.0
        } else {
            self.frames as f64 * 1000.0 / self.elapsed_ms as f64
        }
    }

    /// Whole-session cumulative throughput (pre-window behaviour, fallback).
    fn cumulative_throughput_bps(&self) -> f64 {
        if self.elapsed_ms == 0 {
            0.0
        } else {
            self.bytes as f64 * 1000.0 / self.elapsed_ms as f64
        }
    }

    /// Append a rate sample and evict samples that fall outside the window.
    fn push_sample_at(&mut self, t_ms: u64, bytes: u64) {
        self.window.push_back(RateSample { t_ms, bytes });
        let cutoff = t_ms.saturating_sub(STATS_WINDOW_MS);
        while self.window.len() > 2 && self.window.front().is_some_and(|s| s.t_ms < cutoff) {
            self.window.pop_front();
        }
        while self.window.len() > STATS_MAX_SAMPLES {
            self.window.pop_front();
        }
    }

    /// Over the sliding window ending at `now`, return `(span_ms, frames, bytes)`
    /// where `span_ms` is the wall-clock between the oldest and newest retained
    /// samples and `frames`/`bytes` are the totals across those samples.
    fn window_rates(&self, now: u64) -> (u64, u64, u64) {
        let cutoff = now.saturating_sub(STATS_WINDOW_MS);
        let mut first: Option<u64> = None;
        let mut last: Option<u64> = None;
        let mut frames: u64 = 0;
        let mut bytes: u64 = 0;
        for s in self.window.iter() {
            if s.t_ms < cutoff {
                continue;
            }
            if first.is_none() {
                first = Some(s.t_ms);
            }
            last = Some(s.t_ms);
            frames += 1;
            bytes += s.bytes;
        }
        let span_ms = match (first, last) {
            (Some(f), Some(l)) if l > f => l - f,
            _ => 0,
        };
        (span_ms, frames, bytes)
    }

    /// Estimated remaining time given `remaining_bytes` at current throughput.
    pub fn eta(&self, remaining_bytes: u64) -> Option<Duration> {
        let tput = self.throughput_bps();
        if tput <= 0.0 {
            return None;
        }
        Some(Duration::from_secs_f64(remaining_bytes as f64 / tput))
    }
}

/// Receiver-side recovery progress.
#[derive(Debug, Clone, Default)]
pub struct Progress {
    /// Source symbols decoded so far (sum across blocks).
    pub decoded_symbols: u32,
    /// Total source symbols K.
    pub total_symbols: u32,
    /// Symbol size in bytes (from descriptor / frame header). 0 until known.
    /// Exposed to UIs so wire throughput (receivedΔ × symbol_size / Δt) is
    /// accurate; matches Android's `session.symbolSizeBytes()`.
    pub symbol_size: u32,
    /// Unique symbols received (deduplicated) — may exceed decoded when a
    /// block has not yet hit its decode threshold.
    pub received_symbols: u32,
    /// Distinct frames received (including duplicates) for loss-rate stats.
    pub frames_seen: u64,
    /// Duplicate frames (same ESI received multiple times).
    pub frames_duplicate: u64,
    /// Frames that failed CRC and were discarded.
    pub frames_corrupt: u64,
    /// Number of blocks fully reconstructed.
    pub decoded_blocks: u32,
    /// Total blocks.
    pub total_blocks: u32,
    /// Whether metadata has been confirmed via descriptor frame.
    pub meta_confirmed: bool,
    /// Number of consecutive session-mismatch errors since the last accepted
    /// frame (reset to 0 when a frame is accepted).  Used by the JNI layer to
    /// signal the Kotlin side that the receiver was likely initialised from a
    /// corrupted first QR decode and should be re-created.
    pub session_mismatch_streak: u32,
}

impl Progress {
    /// Fraction (0.0..=1.0) of source symbols decoded.
    pub fn decoded_fraction(&self) -> f64 {
        if self.total_symbols == 0 {
            0.0
        } else {
            self.decoded_symbols as f64 / self.total_symbols as f64
        }
    }

    /// Frame loss ratio (corrupt + duplicate) over total seen.
    ///
    /// Note: frames_seen includes all frames (corrupt, duplicate, and good).
    /// This ratio represents the percentage of frames that were unusable.
    pub fn loss_ratio(&self) -> f64 {
        if self.frames_seen == 0 {
            0.0
        } else {
            (self.frames_duplicate + self.frames_corrupt) as f64 / self.frames_seen as f64
        }
    }

    pub fn is_complete(&self) -> bool {
        self.total_blocks > 0 && self.decoded_blocks == self.total_blocks
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fps_and_throughput() {
        let s = Stats {
            frames: 300,
            bytes: 300 * 1024,
            elapsed_ms: 10_000,
            ..Default::default()
        };
        // Empty window → falls back to the cumulative average.
        assert!((s.fps() - 30.0).abs() < 0.01);
        assert!((s.throughput_bps() - 30.0 * 1024.0).abs() < 0.5);
    }

    /// The whole point of the sliding window: a long-running session that once
    /// stalled (a long gap with no frames) must report the *current* frame rate,
    /// not the diluted cumulative average. Here the session runs 60s of history
    /// (at 30fps), then a 10s stall, then 2s at 60fps. The window must report
    /// ~60fps / ~60KiB/s, not the cumulative ~16fps.
    #[test]
    fn sliding_window_recovers_after_stall() {
        let mut s = Stats::default();
        let symbol_bytes: u64 = 1024;
        // 60s at 30fps → 1800 frames.
        for i in 0..1800u64 {
            s.push_sample_at(i * 1000 / 30, symbol_bytes);
        }
        // A 10s stall: no frames pushed.
        // Then 2s at 60fps → 120 frames.
        for i in 0..120u64 {
            s.push_sample_at(70_000 + i * 1000 / 60, symbol_bytes);
        }
        s.elapsed_ms = 72_000;
        s.frames = 1920;
        s.bytes = 1920 * symbol_bytes;

        let fps = s.fps_at(72_000);
        let bps = s.throughput_bps_at(72_000);
        // Recent 3s window: 120 frames over 2s → ~60fps, ~60 KiB/s.
        assert!((fps - 60.0).abs() < 2.0, "window fps {fps} != ~60");
        assert!(
            (bps - 60.0 * symbol_bytes as f64).abs() < 2.0 * symbol_bytes as f64,
            "window bps {bps} != ~60 KiB/s"
        );
        // The cumulative average would be ~16.7fps — ensure we are far above it.
        assert!(fps > 40.0);
    }

    /// The window must be bounded: pushing far more samples than the window holds
    /// never grows the internal buffer unboundedly.
    #[test]
    fn sliding_window_is_bounded() {
        let mut s = Stats::default();
        for i in 0..10_000u64 {
            s.push_sample_at(i * 10, 1); // 100 µs apart → thousands within window
        }
        assert!(s.window.len() <= STATS_MAX_SAMPLES);
        assert!(s.window.len() <= (STATS_WINDOW_MS / 10 + 2) as usize);
    }

    #[test]
    fn progress_fraction_and_loss() {
        let p = Progress {
            decoded_symbols: 50,
            total_symbols: 100,
            frames_seen: 100,
            frames_duplicate: 10,
            frames_corrupt: 5,
            decoded_blocks: 2,
            total_blocks: 4,
            ..Default::default()
        };
        assert!((p.decoded_fraction() - 0.5).abs() < 1e-9);
        // loss_ratio = (duplicate + corrupt) / seen = (10 + 5) / 100 = 0.15
        assert!((p.loss_ratio() - 0.15).abs() < 1e-9);
        assert!(!p.is_complete());
    }
}
