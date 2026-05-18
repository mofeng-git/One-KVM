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

#[derive(Debug, Clone, Default)]
pub struct AtxControllerConfig {
    pub enabled: bool,
    pub power: AtxKeyConfig,
    pub reset: AtxKeyConfig,
    pub led: AtxLedConfig,
}

/// Grouped together to reduce lock acquisitions
struct AtxInner {
    config: AtxControllerConfig,
    power_executor: Option<AtxKeyExecutor>,
    reset_executor: Option<AtxKeyExecutor>,
    led_sensor: Option<LedSensor>,
}

/// Manages ATX power control through independent executors for each action.
/// Supports hot-reload of configuration.
pub struct AtxController {
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

    async fn init_key_executor(
        warn_label: &str,
        info_label: &str,
        config: AtxKeyConfig,
        mut executor: AtxKeyExecutor,
    ) -> Option<AtxKeyExecutor> {
        if let Err(e) = executor.init().await {
            warn!("Failed to initialize {} executor: {}", warn_label, e);
            return None;
        }

        info!(
            "{} executor initialized: {:?} on {} pin {}",
            info_label, config.driver, config.device, config.pin
        );
        Some(executor)
    }

    async fn init_components(inner: &mut AtxInner) {
        if Self::should_share_serial_device(&inner.config.power, &inner.config.reset) {
            match AtxKeyExecutor::open_shared_serial(
                &inner.config.power.device,
                inner.config.power.baud_rate,
            ) {
                Ok(shared_serial) => {
                    for (slot, warn_label, info_label, config, serial) in [
                        (
                            &mut inner.power_executor,
                            "power",
                            "Power",
                            inner.config.power.clone(),
                            shared_serial.clone(),
                        ),
                        (
                            &mut inner.reset_executor,
                            "reset",
                            "Reset",
                            inner.config.reset.clone(),
                            shared_serial,
                        ),
                    ] {
                        let executor = AtxKeyExecutor::new_with_shared_serial(
                            config.clone(),
                            serial,
                        );
                        *slot = Self::init_key_executor(warn_label, info_label, config, executor)
                            .await;
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
            for (slot, warn_label, info_label, config) in [
                (&mut inner.power_executor, "power", "Power", inner.config.power.clone()),
                (&mut inner.reset_executor, "reset", "Reset", inner.config.reset.clone()),
            ] {
                if config.is_configured() {
                    let executor = AtxKeyExecutor::new(config.clone());
                    *slot = Self::init_key_executor(warn_label, info_label, config, executor)
                        .await;
                }
            }
        }

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
        for (slot, label) in [
            (&mut inner.power_executor, "power"),
            (&mut inner.reset_executor, "reset"),
        ] {
            if let Some(executor) = slot.as_mut() {
                if let Err(e) = executor.shutdown().await {
                    warn!("Failed to shutdown {} executor: {}", label, e);
                }
            }
            *slot = None;
        }

        if let Some(sensor) = inner.led_sensor.as_mut() {
            if let Err(e) = sensor.shutdown().await {
                warn!("Failed to shutdown LED sensor: {}", e);
            }
        }
        inner.led_sensor = None;
    }

    async fn read_power_status(sensor: Option<&LedSensor>) -> PowerStatus {
        let Some(sensor) = sensor else {
            return PowerStatus::Unknown;
        };

        match sensor.read().await {
            Ok(status) => status,
            Err(e) => {
                debug!("Failed to read ATX LED sensor: {}", e);
                PowerStatus::Unknown
            }
        }
    }

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

    pub fn disabled() -> Self {
        Self::new(AtxControllerConfig::default())
    }

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

    pub async fn shutdown(&self) -> Result<()> {
        let mut inner = self.inner.write().await;
        Self::shutdown_components(&mut inner).await;
        info!("ATX controller shutdown complete");
        Ok(())
    }

    pub async fn trigger_power_action(&self, action: AtxAction) -> Result<()> {
        let inner = self.inner.read().await;

        let (executor, duration) = match action {
            AtxAction::Short => (inner.power_executor.as_ref(), timing::SHORT_PRESS),
            AtxAction::Long => (inner.power_executor.as_ref(), timing::LONG_PRESS),
            AtxAction::Reset => (inner.reset_executor.as_ref(), timing::RESET_PRESS),
        };

        let Some(executor) = executor else {
            return Err(AppError::Config(match action {
                AtxAction::Reset => "Reset button not configured for ATX controller",
                _ => "Power button not configured for ATX controller",
            }
            .to_string()));
        };

        executor.pulse(duration).await?;
        Ok(())
    }

    pub async fn power_short(&self) -> Result<()> {
        self.trigger_power_action(AtxAction::Short).await
    }

    pub async fn power_long(&self) -> Result<()> {
        self.trigger_power_action(AtxAction::Long).await
    }

    pub async fn reset(&self) -> Result<()> {
        self.trigger_power_action(AtxAction::Reset).await
    }

    pub async fn power_status(&self) -> PowerStatus {
        let inner = self.inner.read().await;
        Self::read_power_status(inner.led_sensor.as_ref()).await
    }

    pub async fn state(&self) -> AtxState {
        let inner = self.inner.read().await;

        let power_status = Self::read_power_status(inner.led_sensor.as_ref()).await;

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
