use std::sync::atomic::{AtomicU32, Ordering};
use tokio::sync::RwLock;
use tracing::{info, warn};

use crate::utils::LogThrottler;

const LOG_THROTTLE_SECS: u64 = 5;

#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) enum MsdHealthStatus {
    #[default]
    Healthy,
    Error {
        reason: String,
        error_code: String,
    },
}

pub struct MsdHealthMonitor {
    status: RwLock<MsdHealthStatus>,
    throttler: LogThrottler,
    error_count: AtomicU32,
    last_error_code: RwLock<Option<String>>,
}

impl MsdHealthMonitor {
    pub fn with_defaults() -> Self {
        Self {
            status: RwLock::new(MsdHealthStatus::Healthy),
            throttler: LogThrottler::with_secs(LOG_THROTTLE_SECS),
            error_count: AtomicU32::new(0),
            last_error_code: RwLock::new(None),
        }
    }

    pub async fn report_error(&self, reason: &str, error_code: &str) {
        let count = self.error_count.fetch_add(1, Ordering::Relaxed) + 1;

        let error_changed = {
            let last = self.last_error_code.read().await;
            last.as_ref().map(|s| s.as_str()) != Some(error_code)
        };

        let throttle_key = format!("msd_{}", error_code);
        if error_changed || self.throttler.should_log(&throttle_key) {
            warn!(
                "MSD error: {} (code: {}, count: {})",
                reason, error_code, count
            );
        }

        *self.last_error_code.write().await = Some(error_code.to_string());

        *self.status.write().await = MsdHealthStatus::Error {
            reason: reason.to_string(),
            error_code: error_code.to_string(),
        };
    }

    pub async fn report_recovered(&self) {
        let prev_status = self.status.read().await.clone();

        if prev_status != MsdHealthStatus::Healthy {
            let error_count = self.error_count.load(Ordering::Relaxed);
            info!("MSD recovered after {} errors", error_count);

            self.error_count.store(0, Ordering::Relaxed);
            self.throttler.clear_all();
            *self.last_error_code.write().await = None;
            *self.status.write().await = MsdHealthStatus::Healthy;
        }
    }

    #[cfg(test)]
    pub(crate) async fn status(&self) -> MsdHealthStatus {
        self.status.read().await.clone()
    }

    #[cfg(test)]
    pub(crate) fn error_count(&self) -> u32 {
        self.error_count.load(Ordering::Relaxed)
    }

    pub async fn is_error(&self) -> bool {
        matches!(*self.status.read().await, MsdHealthStatus::Error { .. })
    }

    #[cfg(test)]
    pub(crate) async fn is_healthy(&self) -> bool {
        matches!(*self.status.read().await, MsdHealthStatus::Healthy)
    }

    pub async fn reset(&self) {
        self.error_count.store(0, Ordering::Relaxed);
        *self.last_error_code.write().await = None;
        *self.status.write().await = MsdHealthStatus::Healthy;
        self.throttler.clear_all();
    }

    pub async fn error_message(&self) -> Option<String> {
        match &*self.status.read().await {
            MsdHealthStatus::Error { reason, .. } => Some(reason.clone()),
            _ => None,
        }
    }
}

impl Default for MsdHealthMonitor {
    fn default() -> Self {
        Self::with_defaults()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_initial_status() {
        let monitor = MsdHealthMonitor::with_defaults();
        assert!(monitor.is_healthy().await);
        assert!(!monitor.is_error().await);
        assert_eq!(monitor.error_count(), 0);
    }

    #[tokio::test]
    async fn test_report_error() {
        let monitor = MsdHealthMonitor::with_defaults();

        monitor
            .report_error("ConfigFS write failed", "configfs_error")
            .await;

        assert!(monitor.is_error().await);
        assert_eq!(monitor.error_count(), 1);

        if let MsdHealthStatus::Error { reason, error_code } = monitor.status().await {
            assert_eq!(reason, "ConfigFS write failed");
            assert_eq!(error_code, "configfs_error");
        } else {
            panic!("Expected Error status");
        }
    }

    #[tokio::test]
    async fn test_report_recovered() {
        let monitor = MsdHealthMonitor::with_defaults();

        monitor
            .report_error("Image not found", "image_not_found")
            .await;
        assert!(monitor.is_error().await);

        monitor.report_recovered().await;
        assert!(monitor.is_healthy().await);
        assert_eq!(monitor.error_count(), 0);
    }

    #[tokio::test]
    async fn test_error_count_increments() {
        let monitor = MsdHealthMonitor::with_defaults();

        for i in 1..=5 {
            monitor.report_error("Error", "io_error").await;
            assert_eq!(monitor.error_count(), i);
        }
    }

    #[tokio::test]
    async fn test_reset() {
        let monitor = MsdHealthMonitor::with_defaults();

        monitor.report_error("Error", "io_error").await;
        assert!(monitor.is_error().await);

        monitor.reset().await;
        assert!(monitor.is_healthy().await);
        assert_eq!(monitor.error_count(), 0);
    }
}
