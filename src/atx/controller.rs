//! ATX Controller
//!
//! High-level controller for ATX power management with flexible hardware binding.
//! Each action (power short, power long, reset) can be configured independently.

use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use super::executor::{timing, AtxKeyExecutor};
use super::led::LedSensor;
use super::types::{
    AtxAction, AtxDriverType, AtxInputBinding, AtxKeyConfig, AtxOutputBinding, AtxState, HddStatus,
    PowerStatus,
};
use crate::error::{AppError, Result};

#[derive(Debug, Clone, Default)]
pub struct AtxControllerConfig {
    pub enabled: bool,
    pub driver: AtxDriverType,
    pub device: String,
    pub baud_rate: u32,
    pub power: AtxOutputBinding,
    pub reset: AtxOutputBinding,
    pub led: AtxInputBinding,
    pub hdd: AtxInputBinding,
}

/// Grouped together to reduce lock acquisitions
struct AtxInner {
    config: AtxControllerConfig,
    power_executor: Option<AtxKeyExecutor>,
    reset_executor: Option<AtxKeyExecutor>,
    led_sensor: Option<LedSensor>,
    hdd_sensor: Option<LedSensor>,
}

/// Manages ATX power control through independent executors for each action.
/// Supports hot-reload of configuration.
pub struct AtxController {
    inner: RwLock<AtxInner>,
}

impl AtxController {
    fn build_key_config(
        config: &AtxControllerConfig,
        binding: &AtxOutputBinding,
    ) -> Option<AtxKeyConfig> {
        if !binding.is_configured_for(config.driver, &config.device) {
            return None;
        }

        let device = match config.driver {
            AtxDriverType::Gpio => binding.device.clone(),
            AtxDriverType::UsbRelay | AtxDriverType::Serial => config.device.clone(),
            AtxDriverType::None => return None,
        };

        Some(AtxKeyConfig {
            driver: config.driver,
            device,
            pin: binding.pin,
            active_level: binding.active_level,
            baud_rate: config.baud_rate,
        })
    }

    fn runtime_key_configs(
        config: &AtxControllerConfig,
    ) -> (Option<AtxKeyConfig>, Option<AtxKeyConfig>) {
        (
            Self::build_key_config(config, &config.power),
            Self::build_key_config(config, &config.reset),
        )
    }

    fn should_share_serial_device(config: &AtxControllerConfig) -> bool {
        if config.driver != AtxDriverType::Serial || config.device.trim().is_empty() {
            return false;
        }

        config.power.enabled && config.reset.enabled
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
        let (power_config, reset_config) = Self::runtime_key_configs(&inner.config);

        if Self::should_share_serial_device(&inner.config) {
            match AtxKeyExecutor::open_shared_serial(&inner.config.device, inner.config.baud_rate) {
                Ok(shared_serial) => {
                    for (slot, warn_label, info_label, config, serial) in [
                        (
                            &mut inner.power_executor,
                            "power",
                            "Power",
                            power_config.clone(),
                            shared_serial.clone(),
                        ),
                        (
                            &mut inner.reset_executor,
                            "reset",
                            "Reset",
                            reset_config.clone(),
                            shared_serial,
                        ),
                    ] {
                        if let Some(config) = config {
                            let executor =
                                AtxKeyExecutor::new_with_shared_serial(config.clone(), serial);
                            *slot =
                                Self::init_key_executor(warn_label, info_label, config, executor)
                                    .await;
                        }
                    }
                }
                Err(e) => {
                    warn!(
                        "Failed to open shared serial device {} for ATX power/reset: {}",
                        inner.config.device, e
                    );
                }
            }
        } else {
            for (slot, warn_label, info_label, config) in [
                (&mut inner.power_executor, "power", "Power", power_config),
                (&mut inner.reset_executor, "reset", "Reset", reset_config),
            ] {
                if let Some(config) = config {
                    let executor = AtxKeyExecutor::new(config.clone());
                    *slot = Self::init_key_executor(warn_label, info_label, config, executor).await;
                }
            }
        }

        if inner.config.driver == AtxDriverType::Gpio && inner.config.led.is_configured() {
            let mut sensor = LedSensor::new(inner.config.led.clone());
            if let Err(e) = sensor.init().await {
                warn!("Failed to initialize LED sensor: {}", e);
            } else {
                info!(
                    "LED sensor initialized on {} pin {}",
                    inner.config.led.device, inner.config.led.pin
                );
                inner.led_sensor = Some(sensor);
            }
        }

        if inner.config.driver == AtxDriverType::Gpio && inner.config.hdd.is_configured() {
            let mut sensor = LedSensor::new(inner.config.hdd.clone());
            if let Err(e) = sensor.init().await {
                warn!("Failed to initialize HDD sensor: {}", e);
            } else {
                info!(
                    "HDD sensor initialized on {} pin {}",
                    inner.config.hdd.device, inner.config.hdd.pin
                );
                inner.hdd_sensor = Some(sensor);
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

        if let Some(sensor) = inner.hdd_sensor.as_mut() {
            if let Err(e) = sensor.shutdown().await {
                warn!("Failed to shutdown HDD sensor: {}", e);
            }
        }
        inner.hdd_sensor = None;
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

    async fn read_hdd_status(sensor: Option<&LedSensor>) -> HddStatus {
        let Some(sensor) = sensor else {
            return HddStatus::Unknown;
        };

        match sensor.read_active().await {
            Ok(true) => HddStatus::Active,
            Ok(false) => HddStatus::Inactive,
            Err(e) => {
                debug!("Failed to read ATX HDD sensor: {}", e);
                HddStatus::Unknown
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
                hdd_sensor: None,
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
            return Err(AppError::Config(
                match action {
                    AtxAction::Reset => "Reset button not configured for ATX controller",
                    _ => "Power button not configured for ATX controller",
                }
                .to_string(),
            ));
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
        let hdd_status = Self::read_hdd_status(inner.hdd_sensor.as_ref()).await;

        AtxState {
            available: inner.config.enabled,
            driver: if inner.config.enabled {
                inner.config.driver
            } else {
                AtxDriverType::None
            },
            power_configured: inner.power_executor.is_some(),
            reset_configured: inner.reset_executor.is_some(),
            power_status,
            led_supported: inner.led_sensor.is_some(),
            hdd_status,
            hdd_supported: inner.hdd_sensor.is_some(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::atx::{AtxDriverType, AtxOutputBinding};

    #[test]
    fn test_should_share_serial_device_true() {
        let config = AtxControllerConfig {
            driver: AtxDriverType::Serial,
            device: "/dev/ttyUSB0".to_string(),
            baud_rate: 9600,
            power: AtxOutputBinding {
                enabled: true,
                pin: 1,
                ..Default::default()
            },
            reset: AtxOutputBinding {
                enabled: true,
                pin: 2,
                ..Default::default()
            },
            ..Default::default()
        };

        assert!(AtxController::should_share_serial_device(&config));
    }

    #[test]
    fn test_should_share_serial_device_false_when_reset_disabled() {
        let config = AtxControllerConfig {
            driver: AtxDriverType::Serial,
            device: "/dev/ttyUSB0".to_string(),
            baud_rate: 9600,
            power: AtxOutputBinding {
                enabled: true,
                pin: 1,
                ..Default::default()
            },
            reset: AtxOutputBinding {
                enabled: false,
                pin: 2,
                ..Default::default()
            },
            ..Default::default()
        };

        assert!(!AtxController::should_share_serial_device(&config));
    }

    #[test]
    fn test_gpio_runtime_key_uses_binding_device() {
        let config = AtxControllerConfig {
            driver: AtxDriverType::Gpio,
            device: "/dev/ignored".to_string(),
            baud_rate: 9600,
            power: AtxOutputBinding {
                enabled: true,
                device: "/dev/gpiochip1".to_string(),
                pin: 4,
                active_level: super::super::types::ActiveLevel::Low,
            },
            ..Default::default()
        };

        let (power, reset) = AtxController::runtime_key_configs(&config);
        let power = power.unwrap();
        assert!(reset.is_none());
        assert_eq!(power.driver, AtxDriverType::Gpio);
        assert_eq!(power.device, "/dev/gpiochip1");
        assert_eq!(power.pin, 4);
        assert_eq!(power.active_level, super::super::types::ActiveLevel::Low);
    }

    #[test]
    fn test_serial_runtime_key_uses_top_level_device() {
        let config = AtxControllerConfig {
            driver: AtxDriverType::Serial,
            device: "/dev/ttyUSB0".to_string(),
            baud_rate: 115200,
            power: AtxOutputBinding {
                enabled: true,
                device: "/dev/ignored".to_string(),
                pin: 3,
                ..Default::default()
            },
            ..Default::default()
        };

        let (power, _) = AtxController::runtime_key_configs(&config);
        let power = power.unwrap();
        assert_eq!(power.driver, AtxDriverType::Serial);
        assert_eq!(power.device, "/dev/ttyUSB0");
        assert_eq!(power.pin, 3);
        assert_eq!(power.baud_rate, 115200);
    }
}
