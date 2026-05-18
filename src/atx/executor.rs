//! ATX key executor backend selector.

use std::time::Duration;
use tracing::debug;

use super::serial_relay::SerialRelayBackend;
use super::traits::{AtxKeyBackend, AtxKeyBackendContext, SharedSerialHandle};
use super::types::{AtxDriverType, AtxKeyConfig};
use crate::error::{AppError, Result};

pub mod timing {
    use std::time::Duration;

    pub const SHORT_PRESS: Duration = Duration::from_millis(500);
    pub const LONG_PRESS: Duration = Duration::from_millis(5000);
    pub const RESET_PRESS: Duration = Duration::from_millis(500);
}

pub struct AtxKeyExecutor {
    config: AtxKeyConfig,
    backend: Option<Box<dyn AtxKeyBackend>>,
}

impl AtxKeyExecutor {
    pub fn new(config: AtxKeyConfig) -> Self {
        Self::with_context(config, AtxKeyBackendContext::Standalone)
    }

    pub fn new_with_shared_serial(config: AtxKeyConfig, serial_handle: SharedSerialHandle) -> Self {
        Self::with_context(config, AtxKeyBackendContext::SharedSerial(serial_handle))
    }

    pub fn open_shared_serial(device: &str, baud_rate: u32) -> Result<SharedSerialHandle> {
        SerialRelayBackend::open_shared_serial(device, baud_rate)
    }

    fn with_context(config: AtxKeyConfig, context: AtxKeyBackendContext) -> Self {
        let backend = build_backend(&config, context);
        Self { config, backend }
    }

    pub fn is_configured(&self) -> bool {
        self.config.is_configured()
    }

    pub async fn init(&mut self) -> Result<()> {
        if !self.config.is_configured() {
            debug!("ATX key executor not configured, skipping init");
            return Ok(());
        }

        let backend = self.backend.as_mut().ok_or_else(|| {
            AppError::Internal(format!(
                "ATX backend {:?} is unsupported on this platform",
                self.config.driver
            ))
        })?;

        backend.init().await
    }

    pub async fn pulse(&self, duration: Duration) -> Result<()> {
        if !self.is_configured() {
            return Err(AppError::Internal("ATX key not configured".to_string()));
        }

        let backend = self.backend.as_ref().ok_or_else(|| {
            AppError::Internal(format!(
                "ATX backend {:?} is unsupported on this platform",
                self.config.driver
            ))
        })?;

        if !backend.is_initialized() {
            return Err(AppError::Internal("ATX key not initialized".to_string()));
        }

        backend.pulse(duration).await
    }

    pub async fn shutdown(&mut self) -> Result<()> {
        if let Some(backend) = self.backend.as_mut() {
            backend.shutdown().await?;
        }
        Ok(())
    }
}

fn build_backend(
    config: &AtxKeyConfig,
    context: AtxKeyBackendContext,
) -> Option<Box<dyn AtxKeyBackend>> {
    match config.driver {
        AtxDriverType::Serial => Some(match context {
            AtxKeyBackendContext::Standalone => Box::new(SerialRelayBackend::new(config.clone())),
            AtxKeyBackendContext::SharedSerial(handle) => Box::new(
                SerialRelayBackend::new_with_shared_serial(config.clone(), handle),
            ),
        }),
        AtxDriverType::Gpio => build_gpio_backend(config),
        AtxDriverType::UsbRelay => build_hidraw_backend(config),
        AtxDriverType::None => None,
    }
}

#[cfg(unix)]
fn build_gpio_backend(config: &AtxKeyConfig) -> Option<Box<dyn AtxKeyBackend>> {
    Some(Box::new(super::gpio_linux::GpioLinuxBackend::new(
        config.clone(),
    )))
}

#[cfg(not(unix))]
fn build_gpio_backend(_config: &AtxKeyConfig) -> Option<Box<dyn AtxKeyBackend>> {
    Some(Box::new(super::disabled_key::DisabledAtxKeyBackend::new(
        "GPIO ATX backend is only available on Linux",
    )))
}

#[cfg(unix)]
fn build_hidraw_backend(config: &AtxKeyConfig) -> Option<Box<dyn AtxKeyBackend>> {
    Some(Box::new(super::hidraw_linux::HidrawLinuxRelayBackend::new(
        config.clone(),
    )))
}

#[cfg(not(unix))]
fn build_hidraw_backend(_config: &AtxKeyConfig) -> Option<Box<dyn AtxKeyBackend>> {
    Some(Box::new(super::disabled_key::DisabledAtxKeyBackend::new(
        "USB hidraw relay backend is only available on Linux",
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::atx::ActiveLevel;

    #[test]
    fn executor_creation() {
        let config = AtxKeyConfig::default();
        let executor = AtxKeyExecutor::new(config);
        assert!(!executor.is_configured());
    }

    #[test]
    fn executor_with_gpio_config() {
        let config = AtxKeyConfig {
            driver: AtxDriverType::Gpio,
            device: "/dev/gpiochip0".to_string(),
            pin: 5,
            active_level: ActiveLevel::High,
            baud_rate: 9600,
        };
        let executor = AtxKeyExecutor::new(config);
        assert!(executor.is_configured());
    }

    #[test]
    fn executor_with_usb_relay_config() {
        let config = AtxKeyConfig {
            driver: AtxDriverType::UsbRelay,
            device: "/dev/hidraw0".to_string(),
            pin: 1,
            active_level: ActiveLevel::High,
            baud_rate: 9600,
        };
        let executor = AtxKeyExecutor::new(config);
        assert!(executor.is_configured());
    }

    #[test]
    fn executor_with_serial_config() {
        let config = AtxKeyConfig {
            driver: AtxDriverType::Serial,
            device: "/dev/ttyUSB0".to_string(),
            pin: 1,
            active_level: ActiveLevel::High,
            baud_rate: 9600,
        };
        let executor = AtxKeyExecutor::new(config);
        assert!(executor.is_configured());
    }

    #[test]
    fn timing_constants() {
        assert_eq!(timing::SHORT_PRESS.as_millis(), 500);
        assert_eq!(timing::LONG_PRESS.as_millis(), 5000);
        assert_eq!(timing::RESET_PRESS.as_millis(), 500);
    }
}
