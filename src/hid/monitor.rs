//! HID device health monitoring
//!
//! This module provides health monitoring for HID devices, including:
//! - Device connectivity checks
//! - Automatic reconnection on failure
//! - Error tracking and notification
//! - Log throttling to prevent log flooding

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, warn};

use crate::events::{EventBus, SystemEvent};
use crate::utils::LogThrottler;

/// HID health status
#[derive(Debug, Clone, PartialEq)]
pub enum HidHealthStatus {
    /// Device is healthy and operational
    Healthy,
    /// Device has an error, attempting recovery
    Error {
        /// Human-readable error reason
        reason: String,
        /// Error code for programmatic handling
        error_code: String,
        /// Number of recovery attempts made
        retry_count: u32,
    },
    /// Device is disconnected
    Disconnected,
}

impl Default for HidHealthStatus {
    fn default() -> Self {
        Self::Healthy
    }
}

/// HID health monitor configuration
#[derive(Debug, Clone)]
pub struct HidMonitorConfig {
    /// Health check interval in milliseconds
    pub check_interval_ms: u64,
    /// Retry interval when device is lost (milliseconds)
    pub retry_interval_ms: u64,
    /// Maximum retry attempts before giving up (0 = infinite)
    pub max_retries: u32,
    /// Log throttle interval in seconds
    pub log_throttle_secs: u64,
    /// Recovery cooldown in milliseconds (suppress logs after recovery)
    pub recovery_cooldown_ms: u64,
}

impl Default for HidMonitorConfig {
    fn default() -> Self {
        Self {
            check_interval_ms: 1000,
            retry_interval_ms: 1000,
            max_retries: 0, // infinite retry
            log_throttle_secs: 5,
            recovery_cooldown_ms: 1000, // 1 second cooldown after recovery
        }
    }
}

/// HID health monitor
///
/// Monitors HID device health and manages error recovery.
/// Publishes WebSocket events when device status changes.
pub struct HidHealthMonitor {
    /// Current health status
    status: RwLock<HidHealthStatus>,
    /// Event bus for notifications
    events: RwLock<Option<Arc<EventBus>>>,
    /// Log throttler to prevent log flooding
    throttler: LogThrottler,
    /// Configuration
    config: HidMonitorConfig,
    /// Whether monitoring is active (reserved for future use)
    #[allow(dead_code)]
    running: AtomicBool,
    /// Current retry count
    retry_count: AtomicU32,
    /// Last error code (for change detection)
    last_error_code: RwLock<Option<String>>,
    /// Last recovery timestamp (milliseconds since start, for cooldown)
    last_recovery_ms: AtomicU64,
    /// Start instant for timing
    start_instant: Instant,
}

impl HidHealthMonitor {
    /// Create a new HID health monitor with the specified configuration
    pub fn new(config: HidMonitorConfig) -> Self {
        let throttle_secs = config.log_throttle_secs;
        Self {
            status: RwLock::new(HidHealthStatus::Healthy),
            events: RwLock::new(None),
            throttler: LogThrottler::with_secs(throttle_secs),
            config,
            running: AtomicBool::new(false),
            retry_count: AtomicU32::new(0),
            last_error_code: RwLock::new(None),
            last_recovery_ms: AtomicU64::new(0),
            start_instant: Instant::now(),
        }
    }

    /// Create a new HID health monitor with default configuration
    pub fn with_defaults() -> Self {
        Self::new(HidMonitorConfig::default())
    }

    /// Set the event bus for broadcasting state changes
    pub async fn set_event_bus(&self, events: Arc<EventBus>) {
        *self.events.write().await = Some(events);
    }

    /// Report an error from HID operations
    ///
    /// This method is called when an HID operation fails. It:
    /// 1. Updates the health status
    /// 2. Logs the error (with throttling and cooldown respect)
    /// 3. Publishes a WebSocket event if the error is new or changed
    ///
    /// # Arguments
    ///
    /// * `backend` - The HID backend type ("otg" or "ch9329")
    /// * `device` - The device path (if known)
    /// * `reason` - Human-readable error description
    /// * `error_code` - Error code for programmatic handling
    pub async fn report_error(
        &self,
        backend: &str,
        device: Option<&str>,
        reason: &str,
        error_code: &str,
    ) {
        let count = self.retry_count.fetch_add(1, Ordering::Relaxed) + 1;

        // Check if we're in cooldown period after recent recovery
        let current_ms = self.start_instant.elapsed().as_millis() as u64;
        let last_recovery = self.last_recovery_ms.load(Ordering::Relaxed);
        let in_cooldown = last_recovery > 0 && current_ms < last_recovery + self.config.recovery_cooldown_ms;

        // Check if error code changed
        let error_changed = {
            let last = self.last_error_code.read().await;
            last.as_ref().map(|s| s.as_str()) != Some(error_code)
        };

        // Log with throttling (skip if in cooldown period unless error type changed)
        let throttle_key = format!("hid_{}_{}", backend, error_code);
        if !in_cooldown && (error_changed || self.throttler.should_log(&throttle_key)) {
            warn!(
                "HID {} error: {} (code: {}, attempt: {})",
                backend, reason, error_code, count
            );
        }

        // Update last error code
        *self.last_error_code.write().await = Some(error_code.to_string());

        // Update status
        *self.status.write().await = HidHealthStatus::Error {
            reason: reason.to_string(),
            error_code: error_code.to_string(),
            retry_count: count,
        };

        // Publish event (only if error changed or first occurrence, and not in cooldown)
        if !in_cooldown && (error_changed || count == 1) {
            if let Some(ref events) = *self.events.read().await {
                events.publish(SystemEvent::HidDeviceLost {
                    backend: backend.to_string(),
                    device: device.map(|s| s.to_string()),
                    reason: reason.to_string(),
                    error_code: error_code.to_string(),
                });
            }
        }
    }

    /// Report that a reconnection attempt is starting
    ///
    /// Publishes a reconnecting event to notify clients.
    ///
    /// # Arguments
    ///
    /// * `backend` - The HID backend type
    pub async fn report_reconnecting(&self, backend: &str) {
        let attempt = self.retry_count.load(Ordering::Relaxed);

        // Only publish every 5 attempts to avoid event spam
        if attempt == 1 || attempt % 5 == 0 {
            debug!("HID {} reconnecting, attempt {}", backend, attempt);

            if let Some(ref events) = *self.events.read().await {
                events.publish(SystemEvent::HidReconnecting {
                    backend: backend.to_string(),
                    attempt,
                });
            }
        }
    }

    /// Report that the device has recovered
    ///
    /// This method is called when the HID device successfully reconnects.
    /// It resets the error state and publishes a recovery event.
    ///
    /// # Arguments
    ///
    /// * `backend` - The HID backend type
    pub async fn report_recovered(&self, backend: &str) {
        let prev_status = self.status.read().await.clone();

        // Only report recovery if we were in an error state
        if prev_status != HidHealthStatus::Healthy {
            let retry_count = self.retry_count.load(Ordering::Relaxed);

            // Set cooldown timestamp
            let current_ms = self.start_instant.elapsed().as_millis() as u64;
            self.last_recovery_ms.store(current_ms, Ordering::Relaxed);

            // Only log and publish events if there were multiple retries
            // (avoid log spam for transient single-retry recoveries)
            if retry_count > 1 {
                debug!(
                    "HID {} recovered after {} retries",
                    backend, retry_count
                );

                // Publish recovery event
                if let Some(ref events) = *self.events.read().await {
                    events.publish(SystemEvent::HidRecovered {
                        backend: backend.to_string(),
                    });

                    // Also publish state changed to indicate healthy state
                    events.publish(SystemEvent::HidStateChanged {
                        backend: backend.to_string(),
                        initialized: true,
                        error: None,
                        error_code: None,
                    });
                }
            }

            // Reset state (always reset, even for single-retry recoveries)
            self.retry_count.store(0, Ordering::Relaxed);
            *self.last_error_code.write().await = None;
            *self.status.write().await = HidHealthStatus::Healthy;
        }
    }

    /// Get the current health status
    pub async fn status(&self) -> HidHealthStatus {
        self.status.read().await.clone()
    }

    /// Get the current retry count
    pub fn retry_count(&self) -> u32 {
        self.retry_count.load(Ordering::Relaxed)
    }

    /// Check if the monitor is in an error state
    pub async fn is_error(&self) -> bool {
        matches!(*self.status.read().await, HidHealthStatus::Error { .. })
    }

    /// Check if the monitor is healthy
    pub async fn is_healthy(&self) -> bool {
        matches!(*self.status.read().await, HidHealthStatus::Healthy)
    }

    /// Reset the monitor to healthy state without publishing events
    ///
    /// This is useful during initialization.
    pub async fn reset(&self) {
        self.retry_count.store(0, Ordering::Relaxed);
        *self.last_error_code.write().await = None;
        *self.status.write().await = HidHealthStatus::Healthy;
        self.throttler.clear_all();
    }

    /// Get the configuration
    pub fn config(&self) -> &HidMonitorConfig {
        &self.config
    }

    /// Check if we should continue retrying
    ///
    /// Returns `false` if max_retries is set and we've exceeded it.
    pub fn should_retry(&self) -> bool {
        if self.config.max_retries == 0 {
            return true; // Infinite retry
        }
        self.retry_count.load(Ordering::Relaxed) < self.config.max_retries
    }

    /// Get the retry interval
    pub fn retry_interval(&self) -> Duration {
        Duration::from_millis(self.config.retry_interval_ms)
    }
}

impl Default for HidHealthMonitor {
    fn default() -> Self {
        Self::with_defaults()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_initial_status() {
        let monitor = HidHealthMonitor::with_defaults();
        assert!(monitor.is_healthy().await);
        assert!(!monitor.is_error().await);
        assert_eq!(monitor.retry_count(), 0);
    }

    #[tokio::test]
    async fn test_report_error() {
        let monitor = HidHealthMonitor::with_defaults();

        monitor
            .report_error("otg", Some("/dev/hidg0"), "Device not found", "enoent")
            .await;

        assert!(monitor.is_error().await);
        assert_eq!(monitor.retry_count(), 1);

        if let HidHealthStatus::Error {
            reason,
            error_code,
            retry_count,
        } = monitor.status().await
        {
            assert_eq!(reason, "Device not found");
            assert_eq!(error_code, "enoent");
            assert_eq!(retry_count, 1);
        } else {
            panic!("Expected Error status");
        }
    }

    #[tokio::test]
    async fn test_report_recovered() {
        let monitor = HidHealthMonitor::with_defaults();

        // First report an error
        monitor
            .report_error("ch9329", None, "Port not found", "port_not_found")
            .await;
        assert!(monitor.is_error().await);

        // Then report recovery
        monitor.report_recovered("ch9329").await;
        assert!(monitor.is_healthy().await);
        assert_eq!(monitor.retry_count(), 0);
    }

    #[tokio::test]
    async fn test_retry_count_increments() {
        let monitor = HidHealthMonitor::with_defaults();

        for i in 1..=5 {
            monitor
                .report_error("otg", None, "Error", "io_error")
                .await;
            assert_eq!(monitor.retry_count(), i);
        }
    }

    #[tokio::test]
    async fn test_should_retry_infinite() {
        let monitor = HidHealthMonitor::new(HidMonitorConfig {
            max_retries: 0, // infinite
            ..Default::default()
        });

        for _ in 0..100 {
            monitor
                .report_error("otg", None, "Error", "io_error")
                .await;
            assert!(monitor.should_retry());
        }
    }

    #[tokio::test]
    async fn test_should_retry_limited() {
        let monitor = HidHealthMonitor::new(HidMonitorConfig {
            max_retries: 3,
            ..Default::default()
        });

        assert!(monitor.should_retry());

        monitor.report_error("otg", None, "Error", "io_error").await;
        assert!(monitor.should_retry()); // 1 < 3

        monitor.report_error("otg", None, "Error", "io_error").await;
        assert!(monitor.should_retry()); // 2 < 3

        monitor.report_error("otg", None, "Error", "io_error").await;
        assert!(!monitor.should_retry()); // 3 >= 3
    }

    #[tokio::test]
    async fn test_reset() {
        let monitor = HidHealthMonitor::with_defaults();

        monitor
            .report_error("otg", None, "Error", "io_error")
            .await;
        assert!(monitor.is_error().await);

        monitor.reset().await;
        assert!(monitor.is_healthy().await);
        assert_eq!(monitor.retry_count(), 0);
    }
}
