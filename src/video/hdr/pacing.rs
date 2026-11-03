//! Frame decimation for sources faster than the stream (4K60 HDR -> 30 fps).

use std::time::Duration;

pub struct Decimator {
    min_interval: Duration,
    last: Option<Duration>,
}

impl Decimator {
    /// Admits a frame once at least 90% of the target interval has elapsed, so
    /// a 60 Hz source yields every second frame and 29.97 Hz is not thinned.
    pub fn new(fps: u32) -> Self {
        let fps = fps.max(1) as u64;
        Self {
            min_interval: Duration::from_micros(900_000 / fps),
            last: None,
        }
    }

    pub fn reset(&mut self) {
        self.last = None;
    }

    /// `timestamp` is the frame's capture time on a monotonic clock. A time
    /// earlier than the last admitted frame (clock reset, stream restart)
    /// re-anchors the decimator instead of stalling it.
    pub fn admit(&mut self, timestamp: Duration) -> bool {
        match self.last {
            Some(last) if timestamp >= last => {
                if timestamp - last < self.min_interval {
                    false
                } else {
                    self.last = Some(timestamp);
                    true
                }
            }
            _ => {
                self.last = Some(timestamp);
                true
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn admitted(source_hz: f64, target_fps: u32, frames: u32) -> u32 {
        let mut decimator = Decimator::new(target_fps);
        (0..frames)
            .filter(|n| decimator.admit(Duration::from_secs_f64(*n as f64 / source_hz)))
            .count() as u32
    }

    #[test]
    fn sixty_hertz_source_yields_thirty_fps() {
        assert_eq!(admitted(60.0, 30, 600), 300);
        assert_eq!(admitted(59.94, 30, 600), 300);
    }

    #[test]
    fn slower_sources_pass_through() {
        assert_eq!(admitted(30.0, 30, 300), 300);
        assert_eq!(admitted(29.97, 30, 300), 300);
        assert_eq!(admitted(24.0, 30, 240), 240);
    }

    #[test]
    fn first_frame_is_always_admitted() {
        assert!(Decimator::new(30).admit(Duration::ZERO));
        assert!(Decimator::new(30).admit(Duration::from_millis(1)));
    }

    #[test]
    fn a_clock_that_goes_back_re_anchors() {
        let mut decimator = Decimator::new(30);
        assert!(decimator.admit(Duration::from_secs(10)));
        assert!(decimator.admit(Duration::from_secs(1)));
        assert!(!decimator.admit(Duration::from_secs(1) + Duration::from_millis(16)));
        assert!(decimator.admit(Duration::from_secs(1) + Duration::from_millis(33)));
    }

    #[test]
    fn reset_admits_the_next_frame() {
        let mut decimator = Decimator::new(30);
        assert!(decimator.admit(Duration::from_millis(100)));
        decimator.reset();
        assert!(decimator.admit(Duration::from_millis(101)));
    }
}
