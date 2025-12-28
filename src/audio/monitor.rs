//! Audio device health monitoring
//!
//! This module provides health monitoring for audio capture devices, including:
//! - Device connectivity checks
//! - Automatic reconnection on failure
//! - Error tracking and notification
//! - Log throttling to prevent log flooding

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use crate::events::{EventBus, SystemEvent};
use crate::utils::LogThrottler;

/// Audio health status
#[derive(Debug, Clone, PartialEq)]
pub enum AudioHealthStatus {
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
    /// Device is disconnected or not available
    Disconnected,
}

impl Default for AudioHealthStatus {
    fn default() -> Self {
        Self::Healthy
    }
}

/// Audio health monitor configuration
#[derive(Debug, Clone)]
pub struct AudioMonitorConfig {
    /// Retry interval when device is lost (milliseconds)
    pub retry_interval_ms: u64,
    /// Maximum retry attempts before giving up (0 = infinite)
    pub max_retries: u32,
    /// Log throttle interval in seconds
    pub log_throttle_secs: u64,
}

impl Default for AudioMonitorConfig {
    fn default() -> Self {
        Self {
            retry_interval_ms: 1000,
            max_retries: 0, // infinite retry
            log_throttle_secs: 5,
        }
    }
}

/// Audio health monitor
///
/// Monitors audio device health and manages error recovery.
/// Publishes WebSocket events when device status changes.
pub struct AudioHealthMonitor {
    /// Current health status
    status: RwLock<AudioHealthStatus>,
    /// Event bus for notifications
    events: RwLock<Option<Arc<EventBus>>>,
    /// Log throttler to prevent log flooding
    throttler: LogThrottler,
    /// Configuration
    config: AudioMonitorConfig,
    /// Whether monitoring is active (reserved for future use)
    #[allow(dead_code)]
    running: AtomicBool,
    /// Current retry count
    retry_count: AtomicU32,
    /// Last error code (for change detection)
    last_error_code: RwLock<Option<String>>,
}

impl AudioHealthMonitor {
    /// Create a new audio health monitor with the specified configuration
    pub fn new(config: AudioMonitorConfig) -> Self {
        let throttle_secs = config.log_throttle_secs;
        Self {
            status: RwLock::new(AudioHealthStatus::Healthy),
            events: RwLock::new(None),
            throttler: LogThrottler::with_secs(throttle_secs),
            config,
            running: AtomicBool::new(false),
            retry_count: AtomicU32::new(0),
            last_error_code: RwLock::new(None),
        }
    }

    /// Create a new audio health monitor with default configuration
    pub fn with_defaults() -> Self {
        Self::new(AudioMonitorConfig::default())
    }

    /// Set the event bus for broadcasting state changes
    pub async fn set_event_bus(&self, events: Arc<EventBus>) {
        *self.events.write().await = Some(events);
    }

    /// Report an error from audio operations
    ///
    /// This method is called when an audio operation fails. It:
    /// 1. Updates the health status
    /// 2. Logs the error (with throttling)
    /// 3. Publishes a WebSocket event if the error is new or changed
    ///
    /// # Arguments
    ///
    /// * `device` - The audio device name (if known)
    /// * `reason` - Human-readable error description
    /// * `error_code` - Error code for programmatic handling
    pub async fn report_error(&self, device: Option<&str>, reason: &str, error_code: &str) {
        let count = self.retry_count.fetch_add(1, Ordering::Relaxed) + 1;

        // Check if error code changed
        let error_changed = {
            let last = self.last_error_code.read().await;
            last.as_ref().map(|s| s.as_str()) != Some(error_code)
        };

        // Log with throttling (always log if error type changed)
        let throttle_key = format!("audio_{}", error_code);
        if error_changed || self.throttler.should_log(&throttle_key) {
            warn!(
                "Audio error: {} (code: {}, attempt: {})",
                reason, error_code, count
            );
        }

        // Update last error code
        *self.last_error_code.write().await = Some(error_code.to_string());

        // Update status
        *self.status.write().await = AudioHealthStatus::Error {
            reason: reason.to_string(),
            error_code: error_code.to_string(),
            retry_count: count,
        };

        // Publish event (only if error changed or first occurrence)
        if error_changed || count == 1 {
            if let Some(ref events) = *self.events.read().await {
                events.publish(SystemEvent::AudioDeviceLost {
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
    pub async fn report_reconnecting(&self) {
        let attempt = self.retry_count.load(Ordering::Relaxed);

        // Only publish every 5 attempts to avoid event spam
        if attempt == 1 || attempt % 5 == 0 {
            debug!("Audio reconnecting, attempt {}", attempt);

            if let Some(ref events) = *self.events.read().await {
                events.publish(SystemEvent::AudioReconnecting { attempt });
            }
        }
    }

    /// Report that the device has recovered
    ///
    /// This method is called when the audio device successfully reconnects.
    /// It resets the error state and publishes a recovery event.
    ///
    /// # Arguments
    ///
    /// * `device` - The audio device name
    pub async fn report_recovered(&self, device: Option<&str>) {
        let prev_status = self.status.read().await.clone();

        // Only report recovery if we were in an error state
        if prev_status != AudioHealthStatus::Healthy {
            let retry_count = self.retry_count.load(Ordering::Relaxed);
            info!("Audio recovered after {} retries", retry_count);

            // Reset state
            self.retry_count.store(0, Ordering::Relaxed);
            self.throttler.clear("audio_");
            *self.last_error_code.write().await = None;
            *self.status.write().await = AudioHealthStatus::Healthy;

            // Publish recovery event
            if let Some(ref events) = *self.events.read().await {
                events.publish(SystemEvent::AudioRecovered {
                    device: device.map(|s| s.to_string()),
                });
            }
        }
    }

    /// Get the current health status
    pub async fn status(&self) -> AudioHealthStatus {
        self.status.read().await.clone()
    }

    /// Get the current retry count
    pub fn retry_count(&self) -> u32 {
        self.retry_count.load(Ordering::Relaxed)
    }

    /// Check if the monitor is in an error state
    pub async fn is_error(&self) -> bool {
        matches!(*self.status.read().await, AudioHealthStatus::Error { .. })
    }

    /// Check if the monitor is healthy
    pub async fn is_healthy(&self) -> bool {
        matches!(*self.status.read().await, AudioHealthStatus::Healthy)
    }

    /// Reset the monitor to healthy state without publishing events
    ///
    /// This is useful during initialization.
    pub async fn reset(&self) {
        self.retry_count.store(0, Ordering::Relaxed);
        *self.last_error_code.write().await = None;
        *self.status.write().await = AudioHealthStatus::Healthy;
        self.throttler.clear_all();
    }

    /// Get the configuration
    pub fn config(&self) -> &AudioMonitorConfig {
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

    /// Get the current error message if in error state
    pub async fn error_message(&self) -> Option<String> {
        match &*self.status.read().await {
            AudioHealthStatus::Error { reason, .. } => Some(reason.clone()),
            _ => None,
        }
    }
}

impl Default for AudioHealthMonitor {
    fn default() -> Self {
        Self::with_defaults()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_initial_status() {
        let monitor = AudioHealthMonitor::with_defaults();
        assert!(monitor.is_healthy().await);
        assert!(!monitor.is_error().await);
        assert_eq!(monitor.retry_count(), 0);
    }

    #[tokio::test]
    async fn test_report_error() {
        let monitor = AudioHealthMonitor::with_defaults();

        monitor
            .report_error(Some("hw:0,0"), "Device not found", "device_disconnected")
            .await;

        assert!(monitor.is_error().await);
        assert_eq!(monitor.retry_count(), 1);

        if let AudioHealthStatus::Error {
            reason,
            error_code,
            retry_count,
        } = monitor.status().await
        {
            assert_eq!(reason, "Device not found");
            assert_eq!(error_code, "device_disconnected");
            assert_eq!(retry_count, 1);
        } else {
            panic!("Expected Error status");
        }
    }

    #[tokio::test]
    async fn test_report_recovered() {
        let monitor = AudioHealthMonitor::with_defaults();

        // First report an error
        monitor
            .report_error(Some("default"), "Capture failed", "capture_error")
            .await;
        assert!(monitor.is_error().await);

        // Then report recovery
        monitor.report_recovered(Some("default")).await;
        assert!(monitor.is_healthy().await);
        assert_eq!(monitor.retry_count(), 0);
    }

    #[tokio::test]
    async fn test_retry_count_increments() {
        let monitor = AudioHealthMonitor::with_defaults();

        for i in 1..=5 {
            monitor
                .report_error(None, "Error", "io_error")
                .await;
            assert_eq!(monitor.retry_count(), i);
        }
    }

    #[tokio::test]
    async fn test_reset() {
        let monitor = AudioHealthMonitor::with_defaults();

        monitor
            .report_error(None, "Error", "io_error")
            .await;
        assert!(monitor.is_error().await);

        monitor.reset().await;
        assert!(monitor.is_healthy().await);
        assert_eq!(monitor.retry_count(), 0);
    }
}
