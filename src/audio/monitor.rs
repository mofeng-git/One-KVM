//! Audio device health and logging throttle for repeated failures.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use tokio::sync::RwLock;
use tracing::{info, warn};

use crate::utils::LogThrottler;

const LOG_THROTTLE_SECS: u64 = 5;

#[derive(Debug, Clone, PartialEq, Default)]
pub enum AudioHealthStatus {
    #[default]
    Healthy,
    Error {
        reason: String,
        error_code: String,
    },
}

pub struct AudioHealthMonitor {
    status: RwLock<AudioHealthStatus>,
    throttler: LogThrottler,
    retry_count: AtomicU32,
    last_error_code: RwLock<Option<String>>,
    /// Hide `error_message` while a new capture attempt is in flight (internal error state unchanged).
    suppress_display: AtomicBool,
}

impl AudioHealthMonitor {
    pub fn new() -> Self {
        Self {
            status: RwLock::new(AudioHealthStatus::Healthy),
            throttler: LogThrottler::with_secs(LOG_THROTTLE_SECS),
            retry_count: AtomicU32::new(0),
            last_error_code: RwLock::new(None),
            suppress_display: AtomicBool::new(false),
        }
    }

    /// Clears the error string exposed via [`Self::error_message`] until the next outcome (`report_error` or recovery).
    pub fn prepare_retry_attempt(&self) {
        self.suppress_display.store(true, Ordering::Relaxed);
    }

    pub async fn report_error(&self, reason: &str, error_code: &str) {
        self.suppress_display.store(false, Ordering::Relaxed);

        let count = self.retry_count.fetch_add(1, Ordering::Relaxed) + 1;

        let error_changed = {
            let last = self.last_error_code.read().await;
            last.as_ref().map(|s| s.as_str()) != Some(error_code)
        };

        let throttle_key = format!("audio_{}", error_code);
        if error_changed || self.throttler.should_log(&throttle_key) {
            warn!(
                "Audio error: {} (code: {}, attempt: {})",
                reason, error_code, count
            );
        }

        *self.last_error_code.write().await = Some(error_code.to_string());

        *self.status.write().await = AudioHealthStatus::Error {
            reason: reason.to_string(),
            error_code: error_code.to_string(),
        };
    }

    pub async fn report_recovered(&self) {
        let prev_status = self.status.read().await.clone();

        if prev_status != AudioHealthStatus::Healthy {
            let retry_count = self.retry_count.load(Ordering::Relaxed);
            info!("Audio recovered after {} retries", retry_count);

            self.suppress_display.store(false, Ordering::Relaxed);
            self.retry_count.store(0, Ordering::Relaxed);
            self.throttler.clear("audio_");
            *self.last_error_code.write().await = None;
            *self.status.write().await = AudioHealthStatus::Healthy;
        }
    }

    pub async fn reset(&self) {
        self.suppress_display.store(false, Ordering::Relaxed);
        self.retry_count.store(0, Ordering::Relaxed);
        *self.last_error_code.write().await = None;
        *self.status.write().await = AudioHealthStatus::Healthy;
        self.throttler.clear_all();
    }

    pub async fn status(&self) -> AudioHealthStatus {
        self.status.read().await.clone()
    }

    pub fn retry_count(&self) -> u32 {
        self.retry_count.load(Ordering::Relaxed)
    }

    pub async fn is_error(&self) -> bool {
        matches!(*self.status.read().await, AudioHealthStatus::Error { .. })
    }

    pub async fn error_message(&self) -> Option<String> {
        if self.suppress_display.load(Ordering::Relaxed) {
            return None;
        }
        match &*self.status.read().await {
            AudioHealthStatus::Error { reason, .. } => Some(reason.clone()),
            _ => None,
        }
    }
}

impl Default for AudioHealthMonitor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_initial_status() {
        let monitor = AudioHealthMonitor::new();
        assert!(!monitor.is_error().await);
        assert_eq!(monitor.retry_count(), 0);
    }

    #[tokio::test]
    async fn test_report_error() {
        let monitor = AudioHealthMonitor::new();

        monitor
            .report_error("Device not found", "device_disconnected")
            .await;

        assert!(monitor.is_error().await);
        assert_eq!(monitor.retry_count(), 1);

        if let AudioHealthStatus::Error { reason, error_code } = monitor.status().await {
            assert_eq!(reason, "Device not found");
            assert_eq!(error_code, "device_disconnected");
        } else {
            panic!("Expected Error status");
        }
    }

    #[tokio::test]
    async fn test_report_recovered() {
        let monitor = AudioHealthMonitor::new();

        monitor
            .report_error("Capture failed", "capture_error")
            .await;
        assert!(monitor.is_error().await);

        monitor.report_recovered().await;
        assert!(!monitor.is_error().await);
        assert_eq!(monitor.retry_count(), 0);
    }

    #[tokio::test]
    async fn test_retry_count_increments() {
        let monitor = AudioHealthMonitor::new();

        for i in 1..=5 {
            monitor.report_error("Error", "io_error").await;
            assert_eq!(monitor.retry_count(), i);
        }
    }

    #[tokio::test]
    async fn test_reset() {
        let monitor = AudioHealthMonitor::new();

        monitor.report_error("Error", "io_error").await;
        assert!(monitor.is_error().await);

        monitor.reset().await;
        assert!(!monitor.is_error().await);
        assert_eq!(monitor.retry_count(), 0);
    }

    #[tokio::test]
    async fn test_prepare_retry_hides_error_until_next_failure() {
        let monitor = AudioHealthMonitor::new();

        monitor.report_error("bad", "e").await;
        assert_eq!(monitor.error_message().await.as_deref(), Some("bad"));

        monitor.prepare_retry_attempt();
        assert!(monitor.is_error().await);
        assert!(monitor.error_message().await.is_none());

        monitor.report_error("still bad", "e").await;
        assert_eq!(monitor.error_message().await.as_deref(), Some("still bad"));
    }
}
