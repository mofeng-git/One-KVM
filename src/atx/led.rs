//! ATX LED Sensor
//!
//! Reads power LED status from GPIO to determine if the target system is powered on.

use gpio_cdev::{Chip, LineHandle, LineRequestFlags};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use tracing::{debug, info};

use super::types::{AtxLedConfig, PowerStatus};
use crate::error::{AppError, Result};

pub struct LedSensor {
    config: AtxLedConfig,
    handle: Mutex<Option<LineHandle>>,
    initialized: AtomicBool,
}

impl LedSensor {
    pub fn new(config: AtxLedConfig) -> Self {
        Self {
            config,
            handle: Mutex::new(None),
            initialized: AtomicBool::new(false),
        }
    }

    pub async fn init(&mut self) -> Result<()> {
        if !self.config.is_configured() {
            debug!("LED sensor not configured, skipping init");
            return Ok(());
        }

        info!(
            "Initializing LED sensor on {} pin {}",
            self.config.gpio_chip, self.config.gpio_pin
        );

        let mut chip = Chip::new(&self.config.gpio_chip)
            .map_err(|e| AppError::Internal(format!("LED GPIO chip failed: {}", e)))?;

        let line = chip.get_line(self.config.gpio_pin).map_err(|e| {
            AppError::Internal(format!(
                "LED GPIO line {} failed: {}",
                self.config.gpio_pin, e
            ))
        })?;

        let handle = line
            .request(LineRequestFlags::INPUT, 0, "one-kvm-led")
            .map_err(|e| AppError::Internal(format!("LED GPIO request failed: {}", e)))?;

        *self.handle.lock().unwrap() = Some(handle);
        self.initialized.store(true, Ordering::Relaxed);

        debug!("LED sensor initialized successfully");
        Ok(())
    }

    pub async fn read(&self) -> Result<PowerStatus> {
        if !self.config.is_configured() || !self.initialized.load(Ordering::Relaxed) {
            return Ok(PowerStatus::Unknown);
        }

        let guard = self.handle.lock().unwrap();
        match guard.as_ref() {
            Some(handle) => {
                let value = handle
                    .get_value()
                    .map_err(|e| AppError::Internal(format!("LED read failed: {}", e)))?;

                let is_on = if self.config.inverted {
                    value == 0
                } else {
                    value == 1
                };

                Ok(if is_on {
                    PowerStatus::On
                } else {
                    PowerStatus::Off
                })
            }
            None => Ok(PowerStatus::Unknown),
        }
    }

    pub async fn shutdown(&mut self) -> Result<()> {
        *self.handle.lock().unwrap() = None;
        self.initialized.store(false, Ordering::Relaxed);
        debug!("LED sensor shutdown complete");
        Ok(())
    }
}

impl Drop for LedSensor {
    fn drop(&mut self) {
        *self.handle.lock().unwrap() = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_led_sensor_creation() {
        let config = AtxLedConfig::default();
        let sensor = LedSensor::new(config);
        assert!(!sensor.config.is_configured());
        assert!(!sensor.initialized.load(Ordering::Relaxed));
    }

    #[test]
    fn test_led_sensor_with_config() {
        let config = AtxLedConfig {
            enabled: true,
            gpio_chip: "/dev/gpiochip0".to_string(),
            gpio_pin: 7,
            inverted: false,
        };
        let sensor = LedSensor::new(config);
        assert!(sensor.config.is_configured());
        assert!(!sensor.initialized.load(Ordering::Relaxed));
    }

    #[test]
    fn test_led_sensor_inverted_config() {
        let config = AtxLedConfig {
            enabled: true,
            gpio_chip: "/dev/gpiochip0".to_string(),
            gpio_pin: 7,
            inverted: true,
        };
        let sensor = LedSensor::new(config);
        assert!(sensor.config.inverted);
    }
}
