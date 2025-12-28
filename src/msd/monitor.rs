//! MSD (Mass Storage Device) health monitoring
//!
//! This module provides health monitoring for MSD operations, including:
//! - ConfigFS operation error tracking
//! - Image mount/unmount error tracking
//! - Error notification
//! - Log throttling to prevent log flooding

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

use crate::events::{EventBus, SystemEvent};
use crate::utils::LogThrottler;

/// MSD health status
#[derive(Debug, Clone, PartialEq)]
pub enum MsdHealthStatus {
    /// Device is healthy and operational
    Healthy,
    /// Device has an error
    Error {
        /// Human-readable error reason
        reason: String,
        /// Error code for programmatic handling
        error_code: String,
    },
}

impl Default for MsdHealthStatus {
    fn default() -> Self {
        Self::Healthy
    }
}

/// MSD health monitor configuration
#[derive(Debug, Clone)]
pub struct MsdMonitorConfig {
    /// Log throttle interval in seconds
    pub log_throttle_secs: u64,
}

impl Default for MsdMonitorConfig {
    fn default() -> Self {
        Self {
            log_throttle_secs: 5,
        }
    }
}

/// MSD health monitor
///
/// Monitors MSD operation health and manages error notifications.
/// Publishes WebSocket events when operation status changes.
pub struct MsdHealthMonitor {
    /// Current health status
    status: RwLock<MsdHealthStatus>,
    /// Event bus for notifications
    events: RwLock<Option<Arc<EventBus>>>,
    /// Log throttler to prevent log flooding
    throttler: LogThrottler,
    /// Configuration
    #[allow(dead_code)]
    config: MsdMonitorConfig,
    /// Whether monitoring is active (reserved for future use)
    #[allow(dead_code)]
    running: AtomicBool,
    /// Error count (for tracking)
    error_count: AtomicU32,
    /// Last error code (for change detection)
    last_error_code: RwLock<Option<String>>,
}

impl MsdHealthMonitor {
    /// Create a new MSD health monitor with the specified configuration
    pub fn new(config: MsdMonitorConfig) -> Self {
        let throttle_secs = config.log_throttle_secs;
        Self {
            status: RwLock::new(MsdHealthStatus::Healthy),
            events: RwLock::new(None),
            throttler: LogThrottler::with_secs(throttle_secs),
            config,
            running: AtomicBool::new(false),
            error_count: AtomicU32::new(0),
            last_error_code: RwLock::new(None),
        }
    }

    /// Create a new MSD health monitor with default configuration
    pub fn with_defaults() -> Self {
        Self::new(MsdMonitorConfig::default())
    }

    /// Set the event bus for broadcasting state changes
    pub async fn set_event_bus(&self, events: Arc<EventBus>) {
        *self.events.write().await = Some(events);
    }

    /// Report an error from MSD operations
    ///
    /// This method is called when an MSD operation fails. It:
    /// 1. Updates the health status
    /// 2. Logs the error (with throttling)
    /// 3. Publishes a WebSocket event if the error is new or changed
    ///
    /// # Arguments
    ///
    /// * `reason` - Human-readable error description
    /// * `error_code` - Error code for programmatic handling
    pub async fn report_error(&self, reason: &str, error_code: &str) {
        let count = self.error_count.fetch_add(1, Ordering::Relaxed) + 1;

        // Check if error code changed
        let error_changed = {
            let last = self.last_error_code.read().await;
            last.as_ref().map(|s| s.as_str()) != Some(error_code)
        };

        // Log with throttling (always log if error type changed)
        let throttle_key = format!("msd_{}", error_code);
        if error_changed || self.throttler.should_log(&throttle_key) {
            warn!("MSD error: {} (code: {}, count: {})", reason, error_code, count);
        }

        // Update last error code
        *self.last_error_code.write().await = Some(error_code.to_string());

        // Update status
        *self.status.write().await = MsdHealthStatus::Error {
            reason: reason.to_string(),
            error_code: error_code.to_string(),
        };

        // Publish event (only if error changed or first occurrence)
        if error_changed || count == 1 {
            if let Some(ref events) = *self.events.read().await {
                events.publish(SystemEvent::MsdError {
                    reason: reason.to_string(),
                    error_code: error_code.to_string(),
                });
            }
        }
    }

    /// Report that the MSD has recovered from error
    ///
    /// This method is called when an MSD operation succeeds after errors.
    /// It resets the error state and publishes a recovery event.
    pub async fn report_recovered(&self) {
        let prev_status = self.status.read().await.clone();

        // Only report recovery if we were in an error state
        if prev_status != MsdHealthStatus::Healthy {
            let error_count = self.error_count.load(Ordering::Relaxed);
            info!("MSD recovered after {} errors", error_count);

            // Reset state
            self.error_count.store(0, Ordering::Relaxed);
            self.throttler.clear_all();
            *self.last_error_code.write().await = None;
            *self.status.write().await = MsdHealthStatus::Healthy;

            // Publish recovery event
            if let Some(ref events) = *self.events.read().await {
                events.publish(SystemEvent::MsdRecovered);
            }
        }
    }

    /// Get the current health status
    pub async fn status(&self) -> MsdHealthStatus {
        self.status.read().await.clone()
    }

    /// Get the current error count
    pub fn error_count(&self) -> u32 {
        self.error_count.load(Ordering::Relaxed)
    }

    /// Check if the monitor is in an error state
    pub async fn is_error(&self) -> bool {
        matches!(*self.status.read().await, MsdHealthStatus::Error { .. })
    }

    /// Check if the monitor is healthy
    pub async fn is_healthy(&self) -> bool {
        matches!(*self.status.read().await, MsdHealthStatus::Healthy)
    }

    /// Reset the monitor to healthy state without publishing events
    ///
    /// This is useful during initialization.
    pub async fn reset(&self) {
        self.error_count.store(0, Ordering::Relaxed);
        *self.last_error_code.write().await = None;
        *self.status.write().await = MsdHealthStatus::Healthy;
        self.throttler.clear_all();
    }

    /// Get the current error message if in error state
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

        // First report an error
        monitor
            .report_error("Image not found", "image_not_found")
            .await;
        assert!(monitor.is_error().await);

        // Then report recovery
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
