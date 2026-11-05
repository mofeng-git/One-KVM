//! Shared capture recovery policy.
//!
//! Device discovery decides whether an input follows an external source.  The
//! capture layers consume that decision; they must not infer it again from a
//! driver name because doing so makes MJPEG and WebRTC recover differently.

use std::time::Duration;

use super::capture::BridgeContext;
use super::device::VideoControlMode;

const SOURCE_FOLLOWING_RETRY_DELAYS: [Duration; 3] = [
    Duration::from_millis(500),
    Duration::from_secs(1),
    Duration::from_secs(2),
];
const CONFIGURABLE_RETRY_DELAY: Duration = Duration::from_millis(500);
const CONFIGURABLE_RETRY_LIMIT: u32 = 60;

#[derive(Debug, Clone, Copy)]
pub struct CaptureRecoveryPolicy {
    control_mode: VideoControlMode,
}

/// Wait for a source-change edge, falling back to the policy delay when the
/// driver does not expose events. The short slices keep shutdown responsive.
#[cfg(unix)]
pub fn wait_for_source_change(
    bridge: &BridgeContext,
    delay: Duration,
    should_continue: impl Fn() -> bool,
) -> bool {
    use std::time::Instant;

    use super::device::bridge;

    let Some(path) = bridge.subdev_path.as_ref() else {
        return interruptible_sleep(delay, should_continue);
    };
    let Ok(fd) = bridge::open_subdev(path) else {
        return interruptible_sleep(delay, should_continue);
    };
    if bridge::subscribe_source_change(&fd).is_err() {
        return interruptible_sleep(delay, should_continue);
    }

    let deadline = Instant::now() + delay;
    while should_continue() && Instant::now() < deadline {
        let remaining = deadline.saturating_duration_since(Instant::now());
        match bridge::wait_source_change(&fd, remaining.min(Duration::from_millis(250))) {
            Ok(true) => return true,
            Ok(false) => {}
            Err(_) => return false,
        }
    }
    false
}

#[cfg(windows)]
pub fn wait_for_source_change(
    _bridge: &BridgeContext,
    delay: Duration,
    should_continue: impl Fn() -> bool,
) -> bool {
    interruptible_sleep(delay, should_continue)
}

fn interruptible_sleep(delay: Duration, should_continue: impl Fn() -> bool) -> bool {
    use std::time::Instant;

    let deadline = Instant::now() + delay;
    while should_continue() && Instant::now() < deadline {
        std::thread::sleep(
            deadline
                .saturating_duration_since(Instant::now())
                .min(Duration::from_millis(100)),
        );
    }
    false
}

impl CaptureRecoveryPolicy {
    pub const fn new(control_mode: VideoControlMode) -> Self {
        Self { control_mode }
    }

    pub const fn control_mode(self) -> VideoControlMode {
        self.control_mode
    }

    /// Delay after `failed_attempts` consecutive attempts (one-based).
    pub fn retry_delay(self, failed_attempts: u32) -> Duration {
        match self.control_mode {
            VideoControlMode::SourceFollowing => {
                let index = failed_attempts.saturating_sub(1).min(2) as usize;
                SOURCE_FOLLOWING_RETRY_DELAYS[index]
            }
            VideoControlMode::Configurable => CONFIGURABLE_RETRY_DELAY,
        }
    }

    /// Source-following inputs keep probing for as long as they have a
    /// consumer. Configurable/UVC inputs retain the pre-existing finite policy.
    pub const fn should_retry(self, failed_attempts: u32) -> bool {
        match self.control_mode {
            VideoControlMode::SourceFollowing => true,
            VideoControlMode::Configurable => failed_attempts < CONFIGURABLE_RETRY_LIMIT,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_following_uses_capped_backoff_and_never_expires() {
        let policy = CaptureRecoveryPolicy::new(VideoControlMode::SourceFollowing);
        assert_eq!(policy.retry_delay(1), Duration::from_millis(500));
        assert_eq!(policy.retry_delay(2), Duration::from_secs(1));
        assert_eq!(policy.retry_delay(3), Duration::from_secs(2));
        assert_eq!(policy.retry_delay(10_000), Duration::from_secs(2));
        assert!(policy.should_retry(61));
        assert!(policy.should_retry(9_000)); // five hours at the capped delay
    }

    #[test]
    fn configurable_inputs_keep_the_finite_retry_policy() {
        let policy = CaptureRecoveryPolicy::new(VideoControlMode::Configurable);
        assert_eq!(policy.retry_delay(1), Duration::from_millis(500));
        assert!(policy.should_retry(59));
        assert!(!policy.should_retry(60));
    }
}
