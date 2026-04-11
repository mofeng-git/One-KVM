//! ATX Controller
//!
//! High-level controller for ATX power management with flexible hardware binding.
//! Each action (power short, power long, reset) can be configured independently.

use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use super::executor::{timing, AtxKeyExecutor};
use super::led::LedSensor;
use super::types::{AtxAction, AtxKeyConfig, AtxLedConfig, AtxState, PowerStatus};
use crate::error::{AppError, Result};

/// ATX power control configuration
#[derive(Debug, Clone, Default)]
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
    fn should_share_serial_device(power: &AtxKeyConfig, reset: &AtxKeyConfig) -> bool {
        power.is_configured()
            && reset.is_configured()
            && power.driver == super::types::AtxDriverType::Serial
            && reset.driver == super::types::AtxDriverType::Serial
            && !power.device.is_empty()
            && power.device == reset.device
            && power.baud_rate == reset.baud_rate
    }

    async fn init_components(inner: &mut AtxInner) {
        if Self::should_share_serial_device(&inner.config.power, &inner.config.reset) {
            match AtxKeyExecutor::open_shared_serial(
                &inner.config.power.device,
                inner.config.power.baud_rate,
            ) {
                Ok(shared_serial) => {
                    let mut power_executor = AtxKeyExecutor::new_with_shared_serial(
                        inner.config.power.clone(),
                        shared_serial.clone(),
                    );
                    if let Err(e) = power_executor.init().await {
                        warn!("Failed to initialize power executor: {}", e);
                    } else {
                        info!(
                            "Power executor initialized: {:?} on {} pin {}",
                            inner.config.power.driver,
                            inner.config.power.device,
                            inner.config.power.pin
                        );
                        inner.power_executor = Some(power_executor);
                    }

                    let mut reset_executor = AtxKeyExecutor::new_with_shared_serial(
                        inner.config.reset.clone(),
                        shared_serial,
                    );
                    if let Err(e) = reset_executor.init().await {
                        warn!("Failed to initialize reset executor: {}", e);
                    } else {
                        info!(
                            "Reset executor initialized: {:?} on {} pin {}",
                            inner.config.reset.driver,
                            inner.config.reset.device,
                            inner.config.reset.pin
                        );
                        inner.reset_executor = Some(reset_executor);
                    }
                }
                Err(e) => {
                    warn!(
                        "Failed to open shared serial device {} for ATX power/reset: {}",
                        inner.config.power.device, e
                    );
                }
            }
        } else {
            // Initialize power executor
            if inner.config.power.is_configured() {
                let mut executor = AtxKeyExecutor::new(inner.config.power.clone());
                if let Err(e) = executor.init().await {
                    warn!("Failed to initialize power executor: {}", e);
                } else {
                    info!(
                        "Power executor initialized: {:?} on {} pin {}",
                        inner.config.power.driver,
                        inner.config.power.device,
                        inner.config.power.pin
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
                        inner.config.reset.driver,
                        inner.config.reset.device,
                        inner.config.reset.pin
                    );
                    inner.reset_executor = Some(executor);
                }
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
    }

    async fn shutdown_components(inner: &mut AtxInner) {
        if let Some(executor) = inner.power_executor.as_mut() {
            if let Err(e) = executor.shutdown().await {
                warn!("Failed to shutdown power executor: {}", e);
            }
        }
        inner.power_executor = None;

        if let Some(executor) = inner.reset_executor.as_mut() {
            if let Err(e) = executor.shutdown().await {
                warn!("Failed to shutdown reset executor: {}", e);
            }
        }
        inner.reset_executor = None;

        if let Some(sensor) = inner.led_sensor.as_mut() {
            if let Err(e) = sensor.shutdown().await {
                warn!("Failed to shutdown LED sensor: {}", e);
            }
        }
        inner.led_sensor = None;
    }

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

        Self::init_components(&mut inner).await;

        Ok(())
    }

    /// Reload ATX controller configuration
    pub async fn reload(&self, config: AtxControllerConfig) -> Result<()> {
        let mut inner = self.inner.write().await;

        info!("Reloading ATX controller configuration");

        // Shutdown existing components first, then rebuild with new config.
        Self::shutdown_components(&mut inner).await;
        inner.config = config;

        if !inner.config.enabled {
            info!("ATX disabled after reload");
            return Ok(());
        }

        Self::init_components(&mut inner).await;
        info!("ATX controller reloaded");

        Ok(())
    }

    /// Shutdown ATX controller and release all resources
    pub async fn shutdown(&self) -> Result<()> {
        let mut inner = self.inner.write().await;
        Self::shutdown_components(&mut inner).await;
        info!("ATX controller shutdown complete");
        Ok(())
    }

    /// Trigger a power action (short/long/reset)
    pub async fn trigger_power_action(&self, action: AtxAction) -> Result<()> {
        let inner = self.inner.read().await;

        match action {
            AtxAction::Short | AtxAction::Long => {
                if let Some(executor) = &inner.power_executor {
                    let duration = match action {
                        AtxAction::Short => timing::SHORT_PRESS,
                        AtxAction::Long => timing::LONG_PRESS,
                        _ => unreachable!(),
                    };
                    executor.pulse(duration).await?;
                } else {
                    return Err(AppError::Config(
                        "Power button not configured for ATX controller".to_string(),
                    ));
                }
            }
            AtxAction::Reset => {
                if let Some(executor) = &inner.reset_executor {
                    executor.pulse(timing::RESET_PRESS).await?;
                } else {
                    return Err(AppError::Config(
                        "Reset button not configured for ATX controller".to_string(),
                    ));
                }
            }
        }

        Ok(())
    }

    /// Trigger a short power button press
    pub async fn power_short(&self) -> Result<()> {
        self.trigger_power_action(AtxAction::Short).await
    }

    /// Trigger a long power button press
    pub async fn power_long(&self) -> Result<()> {
        self.trigger_power_action(AtxAction::Long).await
    }

    /// Trigger a reset button press
    pub async fn reset(&self) -> Result<()> {
        self.trigger_power_action(AtxAction::Reset).await
    }

    /// Get the current power status using the LED sensor (if configured)
    pub async fn power_status(&self) -> PowerStatus {
        let inner = self.inner.read().await;

        if let Some(sensor) = &inner.led_sensor {
            match sensor.read().await {
                Ok(status) => status,
                Err(e) => {
                    debug!("Failed to read ATX LED sensor: {}", e);
                    PowerStatus::Unknown
                }
            }
        } else {
            PowerStatus::Unknown
        }
    }

    /// Get a snapshot of the ATX state for API responses
    pub async fn state(&self) -> AtxState {
        let inner = self.inner.read().await;

        let power_status = if let Some(sensor) = &inner.led_sensor {
            match sensor.read().await {
                Ok(status) => status,
                Err(e) => {
                    debug!("Failed to read ATX LED sensor: {}", e);
                    PowerStatus::Unknown
                }
            }
        } else {
            PowerStatus::Unknown
        };

        AtxState {
            available: inner.config.enabled,
            power_configured: inner.power_executor.is_some(),
            reset_configured: inner.reset_executor.is_some(),
            power_status,
            led_supported: inner.led_sensor.is_some(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::atx::AtxDriverType;

    #[test]
    fn test_should_share_serial_device_true() {
        let power = AtxKeyConfig {
            driver: AtxDriverType::Serial,
            device: "/dev/ttyUSB0".to_string(),
            pin: 1,
            active_level: super::super::types::ActiveLevel::High,
            baud_rate: 9600,
        };
        let reset = AtxKeyConfig {
            driver: AtxDriverType::Serial,
            device: "/dev/ttyUSB0".to_string(),
            pin: 2,
            active_level: super::super::types::ActiveLevel::High,
            baud_rate: 9600,
        };

        assert!(AtxController::should_share_serial_device(&power, &reset));
    }

    #[test]
    fn test_should_share_serial_device_false_on_different_baud() {
        let power = AtxKeyConfig {
            driver: AtxDriverType::Serial,
            device: "/dev/ttyUSB0".to_string(),
            pin: 1,
            active_level: super::super::types::ActiveLevel::High,
            baud_rate: 9600,
        };
        let reset = AtxKeyConfig {
            driver: AtxDriverType::Serial,
            device: "/dev/ttyUSB0".to_string(),
            pin: 2,
            active_level: super::super::types::ActiveLevel::High,
            baud_rate: 115200,
        };

        assert!(!AtxController::should_share_serial_device(&power, &reset));
    }
}
