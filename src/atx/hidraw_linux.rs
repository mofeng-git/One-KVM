use async_trait::async_trait;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{debug, info};

use super::traits::AtxKeyBackend;
use super::types::{AtxKeyConfig, LCUS_RELAY_MAX_CHANNEL};
use crate::error::{AppError, Result};

const LCUS_RELAY_COMMAND_LEN: usize = 4;

pub struct HidrawLinuxRelayBackend {
    config: AtxKeyConfig,
    handle: Mutex<Option<File>>,
    initialized: AtomicBool,
}

impl HidrawLinuxRelayBackend {
    pub fn new(config: AtxKeyConfig) -> Self {
        Self {
            config,
            handle: Mutex::new(None),
            initialized: AtomicBool::new(false),
        }
    }

    fn validate_config(&self) -> Result<()> {
        if self.config.pin == 0 {
            return Err(AppError::Config(
                "LCUS HID relay channel must be 1-based (>= 1)".to_string(),
            ));
        }
        if self.config.pin > LCUS_RELAY_MAX_CHANNEL as u32 {
            return Err(AppError::Config(format!(
                "LCUS HID relay channel must be <= {}",
                LCUS_RELAY_MAX_CHANNEL
            )));
        }
        Ok(())
    }

    fn send_command(&self, on: bool) -> Result<()> {
        let channel = u8::try_from(self.config.pin).map_err(|_| {
            AppError::Config(format!(
                "LCUS HID relay channel {} exceeds max {}",
                self.config.pin, LCUS_RELAY_MAX_CHANNEL
            ))
        })?;
        if channel == 0 {
            return Err(AppError::Config(
                "LCUS HID relay channel must be 1-based (>= 1)".to_string(),
            ));
        }
        if channel > LCUS_RELAY_MAX_CHANNEL {
            return Err(AppError::Config(format!(
                "LCUS HID relay channel must be <= {}",
                LCUS_RELAY_MAX_CHANNEL
            )));
        }

        let cmd = Self::build_command(channel, on);
        let mut guard = self.handle.lock().unwrap();
        let device = guard
            .as_mut()
            .ok_or_else(|| AppError::Internal("LCUS HID relay not initialized".to_string()))?;

        device
            .write_all(&cmd)
            .map_err(|e| AppError::Internal(format!("LCUS HID relay write failed: {}", e)))?;
        device
            .flush()
            .map_err(|e| AppError::Internal(format!("LCUS HID relay flush failed: {}", e)))?;

        Ok(())
    }

    pub fn build_command(channel: u8, on: bool) -> [u8; LCUS_RELAY_COMMAND_LEN] {
        let state = if on { 1 } else { 0 };
        let checksum = 0xA0u8.wrapping_add(channel).wrapping_add(state);
        [0xA0, channel, state, checksum]
    }
}

#[async_trait]
impl AtxKeyBackend for HidrawLinuxRelayBackend {
    async fn init(&mut self) -> Result<()> {
        self.validate_config()?;

        info!(
            "Initializing LCUS HID relay ATX backend on {} channel {}",
            self.config.device, self.config.pin
        );

        let device = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&self.config.device)
            .map_err(|e| AppError::Internal(format!("LCUS HID relay device open failed: {}", e)))?;

        *self.handle.lock().unwrap() = Some(device);
        self.send_command(false)?;
        self.initialized.store(true, Ordering::Relaxed);

        debug!(
            "LCUS HID relay channel {} configured successfully",
            self.config.pin
        );
        Ok(())
    }

    async fn pulse(&self, duration: Duration) -> Result<()> {
        if !self.is_initialized() {
            return Err(AppError::Internal(
                "LCUS HID relay not initialized".to_string(),
            ));
        }

        self.send_command(true)?;
        sleep(duration).await;
        self.send_command(false)?;
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<()> {
        if self.is_initialized() {
            let _ = self.send_command(false);
        }
        *self.handle.lock().unwrap() = None;
        self.initialized.store(false, Ordering::Relaxed);
        Ok(())
    }

    fn is_initialized(&self) -> bool {
        self.initialized.load(Ordering::Relaxed)
    }
}

impl Drop for HidrawLinuxRelayBackend {
    fn drop(&mut self) {
        if self.is_initialized() {
            let _ = self.send_command(false);
        }
        *self.handle.lock().unwrap() = None;
    }
}

#[cfg(test)]
mod tests {
    use super::HidrawLinuxRelayBackend;

    #[test]
    fn lcus_hid_relay_command_format() {
        assert_eq!(
            HidrawLinuxRelayBackend::build_command(1, true),
            [0xA0, 0x01, 0x01, 0xA2]
        );
        assert_eq!(
            HidrawLinuxRelayBackend::build_command(1, false),
            [0xA0, 0x01, 0x00, 0xA1]
        );
    }
}
