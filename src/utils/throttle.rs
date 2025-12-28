//! Log throttling utility
//!
//! Provides a mechanism to limit how often the same log message is recorded,
//! preventing log flooding when errors occur repeatedly.

use std::collections::HashMap;
use std::sync::RwLock;
use std::time::{Duration, Instant};

/// Log throttler that limits how often the same message is logged
///
/// This is useful for preventing log flooding when errors occur repeatedly,
/// such as when a device is disconnected and reconnection attempts fail.
///
/// # Example
///
/// ```rust
/// use one_kvm::utils::LogThrottler;
///
/// let throttler = LogThrottler::new(Duration::from_secs(5));
///
/// // First call returns true
/// assert!(throttler.should_log("device_error"));
///
/// // Subsequent calls within 5 seconds return false
/// assert!(!throttler.should_log("device_error"));
/// ```
pub struct LogThrottler {
    /// Map of message key to last log time
    last_logged: RwLock<HashMap<String, Instant>>,
    /// Throttle interval
    interval: Duration,
}

impl LogThrottler {
    /// Create a new log throttler with the specified interval
    ///
    /// # Arguments
    ///
    /// * `interval` - The minimum time between log messages for the same key
    pub fn new(interval: Duration) -> Self {
        Self {
            last_logged: RwLock::new(HashMap::new()),
            interval,
        }
    }

    /// Create a new log throttler with interval specified in seconds
    pub fn with_secs(secs: u64) -> Self {
        Self::new(Duration::from_secs(secs))
    }

    /// Check if a message should be logged (not throttled)
    ///
    /// Returns `true` if the message should be logged, `false` if it should be throttled.
    /// If `true` is returned, the internal timestamp is updated.
    ///
    /// # Arguments
    ///
    /// * `key` - A unique identifier for the message type
    pub fn should_log(&self, key: &str) -> bool {
        let now = Instant::now();

        // First check with read lock (fast path)
        {
            let map = self.last_logged.read().unwrap();
            if let Some(last) = map.get(key) {
                if now.duration_since(*last) < self.interval {
                    return false;
                }
            }
        }

        // Update with write lock
        let mut map = self.last_logged.write().unwrap();
        // Double-check after acquiring write lock
        if let Some(last) = map.get(key) {
            if now.duration_since(*last) < self.interval {
                return false;
            }
        }
        map.insert(key.to_string(), now);
        true
    }

    /// Clear throttle state for a specific key
    ///
    /// This should be called when an error condition recovers,
    /// so the next error will be logged immediately.
    ///
    /// # Arguments
    ///
    /// * `key` - The key to clear
    pub fn clear(&self, key: &str) {
        self.last_logged.write().unwrap().remove(key);
    }

    /// Clear all throttle state
    pub fn clear_all(&self) {
        self.last_logged.write().unwrap().clear();
    }

    /// Get the number of tracked keys
    pub fn len(&self) -> usize {
        self.last_logged.read().unwrap().len()
    }

    /// Check if the throttler is empty
    pub fn is_empty(&self) -> bool {
        self.last_logged.read().unwrap().is_empty()
    }
}

impl Default for LogThrottler {
    /// Create a default log throttler with 5 second interval
    fn default() -> Self {
        Self::with_secs(5)
    }
}

/// Macro for throttled warning logging
///
/// # Example
///
/// ```rust
/// use one_kvm::utils::LogThrottler;
/// use one_kvm::warn_throttled;
///
/// let throttler = LogThrottler::default();
/// warn_throttled!(throttler, "my_error", "Error occurred: {}", "details");
/// ```
#[macro_export]
macro_rules! warn_throttled {
    ($throttler:expr, $key:expr, $($arg:tt)*) => {
        if $throttler.should_log($key) {
            tracing::warn!($($arg)*);
        }
    };
}

/// Macro for throttled error logging
#[macro_export]
macro_rules! error_throttled {
    ($throttler:expr, $key:expr, $($arg:tt)*) => {
        if $throttler.should_log($key) {
            tracing::error!($($arg)*);
        }
    };
}

/// Macro for throttled info logging
#[macro_export]
macro_rules! info_throttled {
    ($throttler:expr, $key:expr, $($arg:tt)*) => {
        if $throttler.should_log($key) {
            tracing::info!($($arg)*);
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_should_log_first_call() {
        let throttler = LogThrottler::with_secs(1);
        assert!(throttler.should_log("test_key"));
    }

    #[test]
    fn test_throttling() {
        let throttler = LogThrottler::new(Duration::from_millis(100));

        // First call should succeed
        assert!(throttler.should_log("test_key"));

        // Immediate second call should be throttled
        assert!(!throttler.should_log("test_key"));

        // Wait for throttle to expire
        thread::sleep(Duration::from_millis(150));

        // Should succeed again
        assert!(throttler.should_log("test_key"));
    }

    #[test]
    fn test_different_keys() {
        let throttler = LogThrottler::with_secs(10);

        // Different keys should be independent
        assert!(throttler.should_log("key1"));
        assert!(throttler.should_log("key2"));
        assert!(!throttler.should_log("key1"));
        assert!(!throttler.should_log("key2"));
    }

    #[test]
    fn test_clear() {
        let throttler = LogThrottler::with_secs(10);

        assert!(throttler.should_log("test_key"));
        assert!(!throttler.should_log("test_key"));

        // Clear the key
        throttler.clear("test_key");

        // Should be able to log again
        assert!(throttler.should_log("test_key"));
    }

    #[test]
    fn test_clear_all() {
        let throttler = LogThrottler::with_secs(10);

        assert!(throttler.should_log("key1"));
        assert!(throttler.should_log("key2"));

        throttler.clear_all();

        assert!(throttler.should_log("key1"));
        assert!(throttler.should_log("key2"));
    }

    #[test]
    fn test_default() {
        let throttler = LogThrottler::default();
        assert!(throttler.should_log("test"));
    }

    #[test]
    fn test_len_and_is_empty() {
        let throttler = LogThrottler::with_secs(10);

        assert!(throttler.is_empty());
        assert_eq!(throttler.len(), 0);

        throttler.should_log("key1");
        assert!(!throttler.is_empty());
        assert_eq!(throttler.len(), 1);

        throttler.should_log("key2");
        assert_eq!(throttler.len(), 2);
    }
}
