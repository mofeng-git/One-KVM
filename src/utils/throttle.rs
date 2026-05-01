//! Limits repeated identical log lines (e.g. reconnect failures).

use std::collections::HashMap;
use std::sync::RwLock;
use std::time::{Duration, Instant};

pub struct LogThrottler {
    last_logged: RwLock<HashMap<String, Instant>>,
    interval: Duration,
}

impl LogThrottler {
    pub fn new(interval: Duration) -> Self {
        Self {
            last_logged: RwLock::new(HashMap::new()),
            interval,
        }
    }

    pub fn with_secs(secs: u64) -> Self {
        Self::new(Duration::from_secs(secs))
    }

    /// Returns whether to emit the log line; updates the timestamp when `true`.
    pub fn should_log(&self, key: &str) -> bool {
        let now = Instant::now();

        {
            let map = self.last_logged.read().unwrap();
            if let Some(last) = map.get(key) {
                if now.duration_since(*last) < self.interval {
                    return false;
                }
            }
        }

        let mut map = self.last_logged.write().unwrap();
        if let Some(last) = map.get(key) {
            if now.duration_since(*last) < self.interval {
                return false;
            }
        }
        map.insert(key.to_string(), now);
        true
    }

    /// Call when a condition recovers so the next failure logs immediately.
    pub fn clear(&self, key: &str) {
        self.last_logged.write().unwrap().remove(key);
    }

    pub fn clear_all(&self) {
        self.last_logged.write().unwrap().clear();
    }
}

impl Clone for LogThrottler {
    fn clone(&self) -> Self {
        Self {
            last_logged: RwLock::new(HashMap::new()),
            interval: self.interval,
        }
    }
}

impl Default for LogThrottler {
    fn default() -> Self {
        Self::with_secs(5)
    }
}

#[macro_export]
macro_rules! warn_throttled {
    ($throttler:expr, $key:expr, $($arg:tt)*) => {
        if $throttler.should_log($key) {
            tracing::warn!($($arg)*);
        }
    };
}

#[macro_export]
macro_rules! error_throttled {
    ($throttler:expr, $key:expr, $($arg:tt)*) => {
        if $throttler.should_log($key) {
            tracing::error!($($arg)*);
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

        assert!(throttler.should_log("test_key"));
        assert!(!throttler.should_log("test_key"));

        thread::sleep(Duration::from_millis(150));

        assert!(throttler.should_log("test_key"));
    }

    #[test]
    fn test_different_keys() {
        let throttler = LogThrottler::with_secs(10);

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

        throttler.clear("test_key");

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
}
