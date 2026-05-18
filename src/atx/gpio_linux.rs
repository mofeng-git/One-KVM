use async_trait::async_trait;
use gpio_cdev::{Chip, LineHandle, LineRequestFlags};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{debug, info};

use super::traits::AtxKeyBackend;
use super::types::{ActiveLevel, AtxKeyConfig};
use crate::error::{AppError, Result};

pub struct GpioLinuxBackend {
    config: AtxKeyConfig,
    handle: Mutex<Option<LineHandle>>,
    initialized: AtomicBool,
}

impl GpioLinuxBackend {
    pub fn new(config: AtxKeyConfig) -> Self {
        Self {
            config,
            handle: Mutex::new(None),
            initialized: AtomicBool::new(false),
        }
    }
}

#[async_trait]
impl AtxKeyBackend for GpioLinuxBackend {
    async fn init(&mut self) -> Result<()> {
        info!(
            "Initializing GPIO ATX backend on {} pin {}",
            self.config.device, self.config.pin
        );

        let mut chip = Chip::new(&self.config.device)
            .map_err(|e| AppError::Internal(format!("GPIO chip open failed: {}", e)))?;

        let line = chip.get_line(self.config.pin).map_err(|e| {
            AppError::Internal(format!("GPIO line {} failed: {}", self.config.pin, e))
        })?;

        let initial_value = match self.config.active_level {
            ActiveLevel::High => 0,
            ActiveLevel::Low => 1,
        };

        let handle = line
            .request(LineRequestFlags::OUTPUT, initial_value, "one-kvm-atx")
            .map_err(|e| AppError::Internal(format!("GPIO request failed: {}", e)))?;

        *self.handle.lock().unwrap() = Some(handle);
        self.initialized.store(true, Ordering::Relaxed);
        debug!("GPIO pin {} configured successfully", self.config.pin);
        Ok(())
    }

    async fn pulse(&self, duration: Duration) -> Result<()> {
        if !self.is_initialized() {
            return Err(AppError::Internal("GPIO not initialized".to_string()));
        }

        let (active, inactive) = match self.config.active_level {
            ActiveLevel::High => (1u8, 0u8),
            ActiveLevel::Low => (0u8, 1u8),
        };

        {
            let guard = self.handle.lock().unwrap();
            let handle = guard
                .as_ref()
                .ok_or_else(|| AppError::Internal("GPIO not initialized".to_string()))?;
            handle
                .set_value(active)
                .map_err(|e| AppError::Internal(format!("GPIO set failed: {}", e)))?;
        }

        sleep(duration).await;

        {
            let guard = self.handle.lock().unwrap();
            if let Some(handle) = guard.as_ref() {
                handle.set_value(inactive).ok();
            }
        }

        Ok(())
    }

    async fn shutdown(&mut self) -> Result<()> {
        *self.handle.lock().unwrap() = None;
        self.initialized.store(false, Ordering::Relaxed);
        Ok(())
    }

    fn is_initialized(&self) -> bool {
        self.initialized.load(Ordering::Relaxed)
    }
}

impl Drop for GpioLinuxBackend {
    fn drop(&mut self) {
        *self.handle.lock().unwrap() = None;
    }
}
