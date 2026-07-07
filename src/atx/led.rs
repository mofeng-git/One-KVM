//! ATX LED Sensor
//!
//! Reads power LED status from GPIO to determine if the target system is powered on.

use gpio_cdev::{Chip, LineHandle, LineRequestFlags};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use tracing::{debug, info};

use super::types::{ActiveLevel, AtxInputBinding, PowerStatus};
use crate::error::{AppError, Result};

pub struct LedSensor {
    config: AtxInputBinding,
    handle: Mutex<Option<LineHandle>>,
    initialized: AtomicBool,
}

impl LedSensor {
    pub fn new(config: AtxInputBinding) -> Self {
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
            self.config.device, self.config.pin
        );

        let mut chip = Chip::new(&self.config.device)
            .map_err(|e| AppError::Internal(format!("LED GPIO chip failed: {}", e)))?;

        let line = chip.get_line(self.config.pin).map_err(|e| {
            AppError::Internal(format!("LED GPIO line {} failed: {}", self.config.pin, e))
        })?;

        let handle = line
            .request(LineRequestFlags::INPUT, 0, "one-kvm-led")
            .map_err(|e| AppError::Internal(format!("LED GPIO request failed: {}", e)))?;

        *self.handle.lock().unwrap() = Some(handle);
        self.initialized.store(true, Ordering::Relaxed);

        debug!("LED sensor initialized successfully");
        Ok(())
    }

    pub async fn read_active(&self) -> Result<bool> {
        if !self.config.is_configured() || !self.initialized.load(Ordering::Relaxed) {
            return Err(AppError::Internal(
                "GPIO input sensor not initialized".to_string(),
            ));
        }

        let guard = self.handle.lock().unwrap();
        match guard.as_ref() {
            Some(handle) => {
                let value = handle
                    .get_value()
                    .map_err(|e| AppError::Internal(format!("LED read failed: {}", e)))?;

                let active = match self.config.active_level {
                    ActiveLevel::High => value == 1,
                    ActiveLevel::Low => value == 0,
                };

                Ok(active)
            }
            None => Err(AppError::Internal(
                "GPIO input sensor not initialized".to_string(),
            )),
        }
    }

    pub async fn read(&self) -> Result<PowerStatus> {
        if !self.config.is_configured() || !self.initialized.load(Ordering::Relaxed) {
            return Ok(PowerStatus::Unknown);
        }

        Ok(if self.read_active().await? {
            PowerStatus::On
        } else {
            PowerStatus::Off
        })
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
        let config = AtxInputBinding::default();
        let sensor = LedSensor::new(config);
        assert!(!sensor.config.is_configured());
        assert!(!sensor.initialized.load(Ordering::Relaxed));
    }

    #[test]
    fn test_led_sensor_with_config() {
        let config = AtxInputBinding {
            enabled: true,
            device: "/dev/gpiochip0".to_string(),
            pin: 7,
            active_level: ActiveLevel::High,
        };
        let sensor = LedSensor::new(config);
        assert!(sensor.config.is_configured());
        assert!(!sensor.initialized.load(Ordering::Relaxed));
    }

    #[test]
    fn test_led_sensor_inverted_config() {
        let config = AtxInputBinding {
            enabled: true,
            device: "/dev/gpiochip0".to_string(),
            pin: 7,
            active_level: ActiveLevel::Low,
        };
        let sensor = LedSensor::new(config);
        assert_eq!(sensor.config.active_level, ActiveLevel::Low);
    }
}
