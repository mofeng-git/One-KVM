//! ATX Controller
//!
//! High-level controller for ATX power management with flexible hardware binding.
//! Each action (power short, power long, reset) can be configured independently.

use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use super::executor::{timing, AtxKeyExecutor};
use super::led::LedSensor;
use super::types::{AtxKeyConfig, AtxLedConfig, AtxState, PowerStatus};
use crate::error::{AppError, Result};

/// ATX power control configuration
#[derive(Debug, Clone)]
pub struct AtxControllerConfig {
    /// Whether ATX is enabled
    pub enabled: bool,
    /// Power button configuration (used for both short and long press)
    pub power: AtxKeyConfig,
    /// Reset button configuration
    pub reset: AtxKeyConfig,
    /// LED sensing configuration
    pub led: AtxLedConfig,
}

impl Default for AtxControllerConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            power: AtxKeyConfig::default(),
            reset: AtxKeyConfig::default(),
            led: AtxLedConfig::default(),
        }
    }
}

/// Internal state holding all ATX components
/// Grouped together to reduce lock acquisitions
struct AtxInner {
    config: AtxControllerConfig,
    power_executor: Option<AtxKeyExecutor>,
    reset_executor: Option<AtxKeyExecutor>,
    led_sensor: Option<LedSensor>,
}

/// ATX Controller
///
/// Manages ATX power control through independent executors for each action.
/// Supports hot-reload of configuration.
pub struct AtxController {
    /// Single lock for all internal state to reduce lock contention
    inner: RwLock<AtxInner>,
}

impl AtxController {
    /// Create a new ATX controller with the specified configuration
    pub fn new(config: AtxControllerConfig) -> Self {
        Self {
            inner: RwLock::new(AtxInner {
                config,
                power_executor: None,
                reset_executor: None,
                led_sensor: None,
            }),
        }
    }

    /// Create a disabled ATX controller
    pub fn disabled() -> Self {
        Self::new(AtxControllerConfig::default())
    }

    /// Initialize the ATX controller and its executors
    pub async fn init(&self) -> Result<()> {
        let mut inner = self.inner.write().await;

        if !inner.config.enabled {
            info!("ATX disabled in configuration");
            return Ok(());
        }

        info!("Initializing ATX controller");

        // Initialize power executor
        if inner.config.power.is_configured() {
            let mut executor = AtxKeyExecutor::new(inner.config.power.clone());
            if let Err(e) = executor.init().await {
                warn!("Failed to initialize power executor: {}", e);
            } else {
                info!(
                    "Power executor initialized: {:?} on {} pin {}",
                    inner.config.power.driver, inner.config.power.device, inner.config.power.pin
                );
                inner.power_executor = Some(executor);
            }
        }

        // Initialize reset executor
        if inner.config.reset.is_configured() {
            let mut executor = AtxKeyExecutor::new(inner.config.reset.clone());
            if let Err(e) = executor.init().await {
                warn!("Failed to initialize reset executor: {}", e);
            } else {
                info!(
                    "Reset executor initialized: {:?} on {} pin {}",
                    inner.config.reset.driver, inner.config.reset.device, inner.config.reset.pin
                );
                inner.reset_executor = Some(executor);
            }
        }

        // Initialize LED sensor
        if inner.config.led.is_configured() {
            let mut sensor = LedSensor::new(inner.config.led.clone());
            if let Err(e) = sensor.init().await {
                warn!("Failed to initialize LED sensor: {}", e);
            } else {
                info!(
                    "LED sensor initialized on {} pin {}",
                    inner.config.led.gpio_chip, inner.config.led.gpio_pin
                );
                inner.led_sensor = Some(sensor);
            }
        }

        info!("ATX controller initialized successfully");
        Ok(())
    }

    /// Reload the ATX controller with new configuration
    ///
    /// This is called when configuration changes and supports hot-reload.
    pub async fn reload(&self, new_config: AtxControllerConfig) -> Result<()> {
        info!("Reloading ATX controller with new configuration");

        // Shutdown existing executors
        self.shutdown_internal().await?;

        // Update configuration and re-initialize
        {
            let mut inner = self.inner.write().await;
            inner.config = new_config;
        }

        // Re-initialize
        self.init().await?;

        info!("ATX controller reloaded successfully");
        Ok(())
    }

    /// Get current ATX state (single lock acquisition)
    pub async fn state(&self) -> AtxState {
        let inner = self.inner.read().await;

        let power_status = if let Some(sensor) = inner.led_sensor.as_ref() {
            sensor.read().await.unwrap_or(PowerStatus::Unknown)
        } else {
            PowerStatus::Unknown
        };

        AtxState {
            available: inner.config.enabled,
            power_configured: inner
                .power_executor
                .as_ref()
                .map(|e| e.is_initialized())
                .unwrap_or(false),
            reset_configured: inner
                .reset_executor
                .as_ref()
                .map(|e| e.is_initialized())
                .unwrap_or(false),
            power_status,
            led_supported: inner
                .led_sensor
                .as_ref()
                .map(|s| s.is_initialized())
                .unwrap_or(false),
        }
    }

    /// Get current state as SystemEvent
    pub async fn current_state_event(&self) -> crate::events::SystemEvent {
        let state = self.state().await;
        crate::events::SystemEvent::AtxStateChanged {
            power_status: state.power_status,
        }
    }

    /// Check if ATX is available
    pub async fn is_available(&self) -> bool {
        let inner = self.inner.read().await;
        inner.config.enabled
    }

    /// Check if power button is configured and initialized
    pub async fn is_power_ready(&self) -> bool {
        let inner = self.inner.read().await;
        inner
            .power_executor
            .as_ref()
            .map(|e| e.is_initialized())
            .unwrap_or(false)
    }

    /// Check if reset button is configured and initialized
    pub async fn is_reset_ready(&self) -> bool {
        let inner = self.inner.read().await;
        inner
            .reset_executor
            .as_ref()
            .map(|e| e.is_initialized())
            .unwrap_or(false)
    }

    /// Short press power button (turn on or graceful shutdown)
    pub async fn power_short(&self) -> Result<()> {
        let inner = self.inner.read().await;
        let executor = inner
            .power_executor
            .as_ref()
            .ok_or_else(|| AppError::Internal("Power button not configured".to_string()))?;

        info!(
            "ATX: Short press power button ({}ms)",
            timing::SHORT_PRESS.as_millis()
        );
        executor.pulse(timing::SHORT_PRESS).await
    }

    /// Long press power button (force power off)
    pub async fn power_long(&self) -> Result<()> {
        let inner = self.inner.read().await;
        let executor = inner
            .power_executor
            .as_ref()
            .ok_or_else(|| AppError::Internal("Power button not configured".to_string()))?;

        info!(
            "ATX: Long press power button ({}ms)",
            timing::LONG_PRESS.as_millis()
        );
        executor.pulse(timing::LONG_PRESS).await
    }

    /// Press reset button
    pub async fn reset(&self) -> Result<()> {
        let inner = self.inner.read().await;
        let executor = inner
            .reset_executor
            .as_ref()
            .ok_or_else(|| AppError::Internal("Reset button not configured".to_string()))?;

        info!(
            "ATX: Press reset button ({}ms)",
            timing::RESET_PRESS.as_millis()
        );
        executor.pulse(timing::RESET_PRESS).await
    }

    /// Get current power status from LED sensor
    pub async fn power_status(&self) -> Result<PowerStatus> {
        let inner = self.inner.read().await;
        match inner.led_sensor.as_ref() {
            Some(sensor) => sensor.read().await,
            None => Ok(PowerStatus::Unknown),
        }
    }

    /// Shutdown the ATX controller
    pub async fn shutdown(&self) -> Result<()> {
        info!("Shutting down ATX controller");
        self.shutdown_internal().await?;
        info!("ATX controller shutdown complete");
        Ok(())
    }

    /// Internal shutdown helper
    async fn shutdown_internal(&self) -> Result<()> {
        let mut inner = self.inner.write().await;

        // Shutdown power executor
        if let Some(mut executor) = inner.power_executor.take() {
            executor.shutdown().await.ok();
        }

        // Shutdown reset executor
        if let Some(mut executor) = inner.reset_executor.take() {
            executor.shutdown().await.ok();
        }

        // Shutdown LED sensor
        if let Some(mut sensor) = inner.led_sensor.take() {
            sensor.shutdown().await.ok();
        }

        Ok(())
    }
}

impl Drop for AtxController {
    fn drop(&mut self) {
        debug!("ATX controller dropped");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_controller_config_default() {
        let config = AtxControllerConfig::default();
        assert!(!config.enabled);
        assert!(!config.power.is_configured());
        assert!(!config.reset.is_configured());
        assert!(!config.led.is_configured());
    }

    #[test]
    fn test_controller_creation() {
        let controller = AtxController::disabled();
        assert!(controller.inner.try_read().is_ok());
    }

    #[tokio::test]
    async fn test_controller_disabled_state() {
        let controller = AtxController::disabled();
        let state = controller.state().await;
        assert!(!state.available);
        assert!(!state.power_configured);
        assert!(!state.reset_configured);
    }

    #[tokio::test]
    async fn test_controller_init_disabled() {
        let controller = AtxController::disabled();
        let result = controller.init().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_controller_is_available() {
        let controller = AtxController::disabled();
        assert!(!controller.is_available().await);

        let config = AtxControllerConfig {
            enabled: true,
            ..Default::default()
        };
        let controller = AtxController::new(config);
        assert!(controller.is_available().await);
    }
}
